//! Load test: ten panes stream output through the real daemon and the core, the way ten busy
//! Claude sessions would. Ignored by default; run it with
//! `cargo test --release -p terminescu-ptyd --test load -- --ignored --nocapture`.
//!
//! It prints memory (RSS) and CPU for the core (this process) and the daemon in two phases:
//! a flood of 200k coloured lines per pane, then 15 s of TUI-style redraws, then idle.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use terminescu_core::daemon::DaemonClient;
use terminescu_core::db::Db;
use terminescu_core::launcher::ensure_running;
use terminescu_core::persist;
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, Core, Workspace};
use terminescu_core::views::ViewId;

/// `LOAD_PANES` and `LOAD_LINES` override the defaults of 10 panes and 200k lines.
fn knob(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
const COLS: u16 = 120;
const ROWS: u16 = 40;

/// Kilobytes of a process's physical footprint, as `footprint` counts it: resident plus
/// compressed pages. Plain RSS drops when macOS compresses idle memory.
fn footprint(pid: u32) -> Option<u64> {
    let out = std::process::Command::new("footprint")
        .args(["-p", &pid.to_string()])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    // "name [pid]: 64-bit    Footprint: 34 MB (16384 bytes per page)"
    let line = s.lines().find(|l| l.contains("Footprint:"))?;
    let mut it = line.split("Footprint:").nth(1)?.split_whitespace();
    let n: f64 = it.next()?.parse().ok()?;
    let kb = match it.next()? {
        "KB" => n,
        "MB" => n * 1024.0,
        "GB" => n * 1024.0 * 1024.0,
        _ => n / 1024.0,
    };
    Some(kb as u64)
}

/// Kilobytes of physical footprint and seconds of CPU of a process.
fn usage(pid: u32) -> Option<(u64, f64)> {
    let out = std::process::Command::new("ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let mut it = s.split_whitespace();
    let rss = footprint(pid)?;
    // `time` reads [[dd-]hh:]mm:ss.cc
    let secs = it
        .next()?
        .split(':')
        .fold(0.0, |acc, p| acc * 60.0 + p.parse::<f64>().unwrap_or(0.0));
    Some((rss, secs))
}

#[derive(Default, Clone, Copy)]
struct Peak {
    core: u64,
    daemon: u64,
}

struct Sampler {
    peak: Arc<Mutex<Peak>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Sampler {
    fn start(core: u32, daemon: u32) -> Self {
        let peak = Arc::new(Mutex::new(Peak::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (p, s) = (peak.clone(), stop.clone());
        let handle = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                let mut pk = p.lock().unwrap();
                if let Some((r, _)) = usage(core) {
                    pk.core = pk.core.max(r);
                }
                if let Some((r, _)) = usage(daemon) {
                    pk.daemon = pk.daemon.max(r);
                }
                drop(pk);
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        Self {
            peak,
            stop,
            handle: Some(handle),
        }
    }
    fn finish(mut self) -> Peak {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take().unwrap().join().unwrap();
        *self.peak.lock().unwrap()
    }
}

fn mb(kb: u64) -> String {
    format!("{:.0} MB", kb as f64 / 1024.0)
}

fn report(phase: &str, wall: Duration, core: u32, daemon: u32, before: (f64, f64), peak: Peak) {
    let (c_rss, c_cpu) = usage(core).unwrap();
    let (d_rss, d_cpu) = usage(daemon).unwrap();
    let w = wall.as_secs_f64();
    println!(
        "{phase:<10} {w:>6.1} s | core: peak {:>7}, now {:>7}, cpu {:>4.0}% | daemon: peak {:>7}, now {:>7}, cpu {:>4.0}%",
        mb(peak.core),
        mb(c_rss),
        (c_cpu - before.0) / w * 100.0,
        mb(peak.daemon),
        mb(d_rss),
        (d_cpu - before.1) / w * 100.0,
    );
}

#[test]
#[allow(non_snake_case)]
#[ignore = "load test, run by hand"]
fn ten_busy_panes() {
    let PANES = knob("LOAD_PANES", 10);
    let LINES = knob("LOAD_LINES", 200_000);
    let limit = Duration::from_secs(knob("LOAD_LIMIT_SECS", 600) as u64);
    let dir: PathBuf = std::env::temp_dir().join(format!("tload-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join("ptyd.sock");
    let log = dir.join("ptyd.log");
    let db = Db::open(dir.join("terminescu.sqlite")).unwrap();
    db.local_owner().unwrap();
    let db = Arc::new(Mutex::new(db));
    let saved = persist::load(&db.lock().unwrap()).unwrap();
    let (s2, l2) = (socket.clone(), log.clone());
    let (rt, _) = Runtime::start(
        Core::new(Workspace::default()),
        saved,
        Box::new(|_| {}),
        Box::new(move |events| {
            ensure_running(
                env!("CARGO_BIN_EXE_terminescu-ptyd").as_ref(),
                &s2,
                &l2,
                &["--idle-exit-secs", "3"],
                Duration::from_secs(10),
            )?;
            DaemonClient::connect(&s2, events)
        }),
        Box::new(move |s| persist::save(&db.lock().unwrap(), s).unwrap()),
        false,
    )
    .unwrap();

    let side = rt.workspace().windows[0].side_tabs[0].id.clone();
    for _ in 1..PANES {
        rt.command(Command::CreateTab {
            side_tab_id: side.clone(),
        })
        .unwrap();
    }
    let ws = rt.workspace();
    let panes: Vec<String> = ws.pane_sides().into_iter().map(|(p, _)| p).collect();
    assert_eq!(panes.len(), PANES);

    // Each view acknowledges what it receives from another thread, as the webview does once
    // xterm.js has written a batch. `tail` keeps the last bytes, to spot the end marker.
    let (ack_tx, ack_rx) = mpsc::channel::<(usize, u64)>();
    let ids: Arc<Mutex<Vec<Option<ViewId>>>> = Arc::new(Mutex::new(vec![None; PANES]));
    let tails: Arc<Vec<Mutex<Vec<u8>>>> = Arc::new((0..PANES).map(|_| Mutex::default()).collect());
    let received = Arc::new(AtomicU64::new(0));
    for (i, p) in panes.iter().enumerate() {
        let (tx, tails, received) = (ack_tx.clone(), tails.clone(), received.clone());
        let sink: terminescu_core::views::Sink = Arc::new(move |batch: Vec<u8>| {
            received.fetch_add(batch.len() as u64, Ordering::Relaxed);
            let mut t = tails[i].lock().unwrap();
            t.extend_from_slice(&batch);
            let cut = t.len().saturating_sub(4096);
            t.drain(..cut);
            tx.send((i, batch.len() as u64)).is_ok()
        });
        let v = rt.open_view(p, COLS, ROWS, sink).unwrap();
        ids.lock().unwrap()[i] = Some(v);
    }
    {
        let (rt, ids) = (rt.clone(), ids.clone());
        std::thread::spawn(move || {
            while let Ok((i, n)) = ack_rx.recv() {
                if let Some(v) = ids.lock().unwrap()[i] {
                    rt.view_ack(v, n);
                }
            }
        });
    }
    let done = |marker: &str| {
        tails
            .iter()
            .all(|t| String::from_utf8_lossy(&t.lock().unwrap()).contains(marker))
    };
    let wait = |marker: &str, limit: Duration| {
        let start = Instant::now();
        while !done(marker) {
            if start.elapsed() > limit {
                for (i, t) in tails.iter().enumerate() {
                    let t = t.lock().unwrap();
                    let tail =
                        String::from_utf8_lossy(&t[t.len().saturating_sub(160)..]).into_owned();
                    println!("pane {i}: {:?} {tail:?}", rt.view_state(&panes[i]));
                }
                panic!("timed out waiting for {marker}");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    let type_all = |cmd: &str| {
        for (i, p) in panes.iter().enumerate() {
            let v = ids.lock().unwrap()[i].unwrap();
            let _ = p;
            rt.view_input(v, format!("{cmd}\r").into_bytes());
        }
    };

    // Wait for every shell's prompt, then quiet them so prompts do not count.
    type_all("printf '%s\\n' READY''MARK");
    wait("READYMARK", Duration::from_secs(30));
    std::thread::sleep(Duration::from_millis(500));

    let core = std::process::id();
    let daemon = {
        let out = std::process::Command::new("pgrep")
            .args(["-f", socket.to_str().unwrap()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap()
    };
    let (c0, _) = usage(core).unwrap();
    let (d0, _) = usage(daemon).unwrap();
    println!(
        "\n{PANES} panes at {COLS}x{ROWS}, idle: core {}, daemon {}",
        mb(c0),
        mb(d0)
    );

    // Phase 1: 200k coloured lines of ~120 columns per pane, all at once.
    let cpu = |pid| usage(pid).unwrap().1;
    let before = (cpu(core), cpu(daemon));
    let sampler = Sampler::start(core, daemon);
    let start = Instant::now();
    let r0 = received.load(Ordering::Relaxed);
    type_all(&format!(
        "perl -e 'for $i (1..{LINES}) {{ printf \"\\e[38;5;%dm%07d\\e[0m \\e[1m%s\\e[0m %s\\n\", 16 + $i % 216, $i, \"Read\", \"x\" x 100 }}'; printf '%s\\n' FLOOD''DONE"
    ));
    wait("FLOODDONE", limit);
    let wall = start.elapsed();
    let peak = sampler.finish();
    report("flood", wall, core, daemon, before, peak);
    println!(
        "           {:.0} MB reached the views ({:.0} MB/s); stale views redraw from snapshots",
        (received.load(Ordering::Relaxed) - r0) as f64 / 1e6,
        (received.load(Ordering::Relaxed) - r0) as f64 / 1e6 / wall.as_secs_f64()
    );

    // Phase 2: a TUI redrawing a 30-line block 30 times a second, like Claude Code's live area.
    let before = (cpu(core), cpu(daemon));
    let sampler = Sampler::start(core, daemon);
    let start = Instant::now();
    type_all(
        "perl -e '$|=1; for $f (1..450) { print \"\\e[30A\" if $f > 1; for $l (1..30) { printf \"\\e[2K\\e[38;5;%dm* %s %d\\e[0m\\n\", 16 + ($f + $l) % 216, \"y\" x 100, $f } select(undef,undef,undef,1/30) }'; printf '%s\\n' TUI''DONE",
    );
    wait("TUIDONE", Duration::from_secs(120));
    let wall = start.elapsed();
    let peak = sampler.finish();
    report("redraw", wall, core, daemon, before, peak);

    // Phase 3: idle, to see what stays.
    let before = (cpu(core), cpu(daemon));
    let sampler = Sampler::start(core, daemon);
    let start = Instant::now();
    std::thread::sleep(Duration::from_secs(5));
    let peak = sampler.finish();
    report("idle", start.elapsed(), core, daemon, before, peak);

    // Stop the runtime first, or its reconnect starts a fresh daemon once this one goes.
    rt.stop();
    std::thread::sleep(Duration::from_millis(500));
    for _ in 0..2 {
        let _ = std::process::Command::new("pkill")
            .args(["-f", socket.to_str().unwrap()])
            .status();
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
