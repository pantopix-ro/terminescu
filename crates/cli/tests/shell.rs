//! The shell integration end to end (D9, D35): real zsh, bash and fish in an in-process daemon,
//! with the real `terminescu` CLI loading variables.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use terminescu_protocol::client::Client;
use terminescu_protocol::{Control, Frame, PaneId, PaneInfo, SpawnSpec, VarMode};

static NEXT: AtomicU32 = AtomicU32::new(0);
const SECRET: &str = "tok-example-shell-9";

struct Session {
    dir: PathBuf,
    home: PathBuf,
    client: Client,
    pane: PaneId,
    /// Everything the pane printed: the scrollback, as the app sees it.
    output: Vec<u8>,
    req: u64,
}

impl Session {
    fn start(shell: &str) -> Self {
        // Short names: socket paths must stay under 104 bytes.
        let dir = std::env::temp_dir().join(format!(
            "tsh-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        // An empty rc keeps zsh from offering its new-user setup.
        std::fs::write(home.join(".zshrc"), "").unwrap();
        let socket = dir.join("ptyd.sock");
        let opts = terminescu_ptyd::server::Options {
            lock: dir.join("ptyd.lock"),
            core_socket: dir.join("core.sock"),
            socket: socket.clone(),
            cli: Some(env!("CARGO_BIN_EXE_terminescu").into()),
            idle: Duration::from_secs(60),
        };
        std::thread::spawn(move || terminescu_ptyd::server::run(opts));
        let start = Instant::now();
        let mut client = loop {
            if let Ok(c) = Client::connect(&socket, "test") {
                break c;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "daemon socket");
            std::thread::sleep(Duration::from_millis(20));
        };
        client
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let home_s = home.to_str().unwrap().to_string();
        let spec = SpawnSpec {
            cols: 120,
            rows: 40,
            cwd: Some(home_s.clone()),
            argv: None,
            env: vec![
                ("SHELL".into(), shell.into()),
                ("HOME".into(), home_s.clone()),
                ("ZDOTDIR".into(), home_s),
                ("BASH_SILENCE_DEPRECATION_WARNING".into(), "1".into()),
            ],
            side_tab: Some("st-1".into()),
        };
        client
            .send(&Frame::Control(Control::Spawn { req: 1, spec }))
            .unwrap();
        let pane = client
            .recv_until(|f| match f {
                Frame::Control(Control::Spawned { req: 1, pane }) => Some(*pane),
                Frame::Control(Control::Error { message, .. }) => panic!("spawn: {message}"),
                _ => None,
            })
            .unwrap();
        client
            .send(&Frame::Control(Control::Attach { pane }))
            .unwrap();
        let mut s = Self {
            dir,
            home,
            client,
            pane,
            output: Vec::new(),
            req: 10,
        };
        s.wait_output(b"\x1b]133;A;src=terminescu");
        s
    }

    /// Reads frames, keeping the pane's output, until `want` answers.
    fn pump<T>(&mut self, mut want: impl FnMut(&Frame) -> Option<T>) -> T {
        let pane = self.pane;
        let output = &mut self.output;
        self.client
            .recv_until(|f| {
                if let Frame::Output { pane: p, data } = f
                    && *p == pane
                {
                    output.extend_from_slice(data);
                }
                want(f)
            })
            .unwrap()
    }

    fn wait_output(&mut self, needle: &[u8]) {
        let start = Instant::now();
        while !self.output.windows(needle.len()).any(|w| w == needle) {
            assert!(
                start.elapsed() < Duration::from_secs(15),
                "no {:?} in {}",
                String::from_utf8_lossy(needle),
                String::from_utf8_lossy(&self.output)
            );
            self.pump(|f| matches!(f, Frame::Output { .. }).then_some(()));
        }
    }

    fn prompts(&self) -> usize {
        let mark = b"\x1b]133;A;src=terminescu";
        self.output
            .windows(mark.len())
            .filter(|w| w == mark)
            .count()
    }

    fn info(&mut self) -> PaneInfo {
        self.req += 1;
        let req = self.req;
        self.client
            .send(&Frame::Control(Control::List { req }))
            .unwrap();
        let pane = self.pane;
        self.pump(|f| match f {
            Frame::Control(Control::Panes { req: r, panes }) if *r == req => {
                panes.iter().find(|p| p.pane == pane).cloned()
            }
            _ => None,
        })
    }

    fn type_line(&mut self, line: &str) {
        self.client
            .send(&Frame::Input {
                pane: self.pane,
                data: format!("{line}\r").into_bytes(),
            })
            .unwrap();
    }

    fn var(&mut self, request: Control) {
        self.client.send(&Frame::Control(request)).unwrap();
        self.pump(|f| match f {
            Frame::Control(Control::Done { .. }) => Some(()),
            Frame::Control(Control::Error { message, .. }) => panic!("{message}"),
            _ => None,
        });
    }

    fn read_home(&self, name: &str) -> String {
        std::fs::read_to_string(self.home.join(name)).unwrap()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self
            .client
            .send(&Frame::Control(Control::Close { pane: self.pane }));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn check_shell(shell: &str) {
    let mut s = Session::start(shell);

    s.type_line("cd /tmp && false");
    let start = Instant::now();
    let info = loop {
        let info = s.info();
        if info.shell.last_exit.is_some() && !info.shell.running {
            break info;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "{info:?}");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(info.shell.integrated);
    assert_eq!(info.shell.last_exit, Some(1), "{shell}");
    assert_eq!(
        info.shell.command.as_deref(),
        Some("cd /tmp && false"),
        "{shell}"
    );
    assert_eq!(info.shell.cwd.as_deref(), Some("/tmp"), "{shell}");

    // An empty line runs nothing: the last command stays reported.
    let prompts = s.prompts();
    s.type_line("");
    let start = Instant::now();
    while s.prompts() == prompts {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{shell}: no new prompt"
        );
        s.pump(|f| matches!(f, Frame::Output { .. }).then_some(()));
    }
    let after_empty = s.info().shell;
    assert_eq!(after_empty, info.shell, "{shell}");

    // The prompt is already up when the secret arrives: the next command still gets it.
    s.var(Control::VarSet {
        req: 2,
        pane: s.pane,
        name: "API_TOKEN".into(),
        value: SECRET.into(),
        mode: VarMode::Secret,
        ttl_secs: None,
    });
    s.type_line("env > \"$HOME/env1.txt\"; echo done:$((6*7))");
    s.wait_output(b"done:42");
    assert!(
        s.read_home("env1.txt")
            .lines()
            .any(|l| l == format!("API_TOKEN={SECRET}")),
        "{shell}: secret missing from the command's environment"
    );

    s.var(Control::VarUnset {
        req: 3,
        pane: s.pane,
        name: "API_TOKEN".into(),
    });
    s.type_line("env > \"$HOME/env2.txt\"; echo done:$((7*7))");
    s.wait_output(b"done:49");
    assert!(
        !s.read_home("env2.txt").contains("API_TOKEN="),
        "{shell}: an unset variable stayed in the shell"
    );
    let path_line = s
        .read_home("env2.txt")
        .lines()
        .find(|l| l.starts_with("PATH="))
        .unwrap()
        .to_string();
    let cli_dir = Path::new(env!("CARGO_BIN_EXE_terminescu"))
        .parent()
        .unwrap();
    assert!(
        path_line.starts_with(&format!("PATH={}:", cli_dir.display())),
        "{shell}: {path_line}"
    );

    assert!(
        !s.output
            .windows(SECRET.len())
            .any(|w| w == SECRET.as_bytes()),
        "{shell}: the secret reached the scrollback"
    );
}

#[test]
fn zsh_reports_commands_and_loads_variables() {
    check_shell("/bin/zsh");
}

#[test]
fn bash_reports_commands_and_loads_variables() {
    check_shell("/bin/bash");
}

#[test]
fn fish_reports_commands_and_loads_variables() {
    let fish = ["/opt/homebrew/bin/fish", "/usr/local/bin/fish"]
        .into_iter()
        .find(|p| Path::new(p).exists());
    match fish {
        Some(fish) => check_shell(fish),
        None => eprintln!("fish is not installed; skipping"),
    }
}

/// Task 6.2: Claude's Bash tool sources `CLAUDE_ENV_FILE` before each command. A secret set
/// after Claude started reaches that command through the `var env` line the `SessionStart` hook
/// wrote, and the value never lands in the file or the scrollback (D35).
#[test]
fn claude_bash_sees_a_secret_through_the_env_file() {
    let mut s = Session::start("/bin/zsh");
    s.type_line(
        "printf '%s' '{\"hook_event_name\":\"SessionStart\",\"session_id\":\"sess-0b5c\"}' \
         | CLAUDE_ENV_FILE=\"$HOME/claude-env.sh\" terminescu hook; echo done:$((5*5))",
    );
    s.wait_output(b"done:25");
    s.var(Control::SideVarSet {
        req: 2,
        side_tab: "st-1".into(),
        name: "API_TOKEN".into(),
        value: SECRET.into(),
        mode: VarMode::Secret,
        ttl_secs: None,
    });
    // `env -u` stands for a Claude started before the secret existed: it inherited neither the
    // value nor the generation that carries it, so only the file brings it.
    s.type_line(
        "env -u API_TOKEN -u TERMINESCU_VARS -u TERMINESCU_VARS_GEN bash -c 'source \"$HOME/claude-env.sh\"; \
         printf %s \"$API_TOKEN\" > \"$HOME/seen.txt\"'; echo done:$((9*9))",
    );
    s.wait_output(b"done:81");
    assert_eq!(s.read_home("seen.txt"), SECRET);
    let file = s.read_home("claude-env.sh");
    assert!(file.contains(" var env "), "{file}");
    assert!(!file.contains(SECRET), "the env file holds the value");
    assert!(
        !s.output
            .windows(SECRET.len())
            .any(|w| w == SECRET.as_bytes()),
        "the secret reached the scrollback"
    );
}
