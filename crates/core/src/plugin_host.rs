//! The plugin host (add-plugin-architecture D5). Each plugin gets its own worker thread and a
//! bounded queue: the core hands an event over and moves on, so a slow or stuck plugin never
//! holds up a patch or a keystroke. Handlers run under `catch_unwind` and their budgets; 5
//! failures in a minute pause the plugin until it is switched off and on.

use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};
use terminescu_plugin_api::{
    AgentAdapter, CoreEvent, Decoration, Id, Manifest, PaneInfo, Plugin, PluginCx, PluginError,
    PluginInfo, Target,
};

/// Events waiting per plugin; a full queue drops its oldest.
pub const QUEUE: usize = 256;
/// Failures in [`WINDOW`] that pause a plugin.
pub const MAX_FAILURES: usize = 5;
const WINDOW: Duration = Duration::from_secs(60);

/// Makes the facade a plugin acts through, for its id.
pub type CxFactory = Arc<dyn Fn(&'static str) -> Arc<dyn PluginCx> + Send + Sync>;
/// Hears that a plugin was paused or switched, for the UI and for clearing decorations.
pub type SwitchSink = Arc<dyn Fn(&str, bool) + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Default)]
struct Queue {
    events: Mutex<VecDeque<CoreEvent>>,
    ready: Condvar,
}

struct Slot {
    plugin: Arc<dyn Plugin>,
    manifest: Manifest,
    cx: Arc<dyn PluginCx>,
    queue: Arc<Queue>,
    enabled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    failures: Mutex<VecDeque<Instant>>,
    /// When the handler now running started; a watchdog pauses a plugin stuck in one.
    running: Mutex<Option<Instant>>,
}

/// A handler this many times over its event budget (and at least [`HANG_MIN`]) counts as hung.
const HANG_FACTOR: u32 = 10;
#[cfg(not(test))]
const HANG_MIN: Duration = Duration::from_secs(5);
#[cfg(test)]
const HANG_MIN: Duration = Duration::from_millis(100);

impl Slot {
    fn live(&self) -> bool {
        self.enabled.load(Ordering::SeqCst) && !self.paused.load(Ordering::SeqCst)
    }

    /// Counts a panic or an overrun; true when it paused the plugin.
    fn fail(&self, what: &str) -> bool {
        eprintln!("terminescu: plugin {} {what}", self.manifest.id);
        let now = Instant::now();
        let mut f = lock(&self.failures);
        f.push_back(now);
        while f.front().is_some_and(|t| now.duration_since(*t) > WINDOW) {
            f.pop_front();
        }
        if f.len() >= MAX_FAILURES && !self.paused.swap(true, Ordering::SeqCst) {
            eprintln!(
                "terminescu: plugin {} paused after {MAX_FAILURES} failures in a minute",
                self.manifest.id
            );
            lock(&self.queue.events).clear();
            return true;
        }
        false
    }
}

type SharedSink = Arc<Mutex<Option<SwitchSink>>>;

fn tell(sink: &SharedSink, id: &str, live: bool) {
    let sink = lock(sink).clone();
    if let Some(sink) = sink {
        sink(id, live);
    }
}

/// The facade a plugin gets: the host's own, except that a plugin that is off or paused cannot
/// add decorations, so a handler still running at the switch cannot undo the clean-up.
struct Guarded {
    inner: Arc<dyn PluginCx>,
    enabled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl PluginCx for Guarded {
    fn panes(&self) -> Vec<PaneInfo> {
        self.inner.panes()
    }
    fn workspace(&self) -> serde_json::Value {
        self.inner.workspace()
    }
    fn side_tabs(&self, window: &str) -> Vec<Id> {
        self.inner.side_tabs(window)
    }
    fn add_context_item(&self, side_tab: &str, text: &str) -> Result<(), PluginError> {
        self.inner.add_context_item(side_tab, text)
    }
    fn add_touched_file(
        &self,
        side_tab: &str,
        tab: Option<&str>,
        path: &str,
        new: bool,
    ) -> Result<(), PluginError> {
        self.inner.add_touched_file(side_tab, tab, path, new)
    }
    fn set_decoration(&self, decoration: Decoration) {
        if self.enabled.load(Ordering::SeqCst) && !self.paused.load(Ordering::SeqCst) {
            self.inner.set_decoration(decoration);
        }
    }
    fn clear_decorations(&self, target: Option<&Target>) {
        self.inner.clear_decorations(target)
    }
    fn open_text(&self, side_tab: &str, path: &str) -> Result<(), PluginError> {
        self.inner.open_text(side_tab, path)
    }
    fn type_in_new_tab(&self, side_tab: &str, cwd: &str, line: &str) -> Result<(), PluginError> {
        self.inner.type_in_new_tab(side_tab, cwd, line)
    }
    fn new_row(&self, a: &str, b: Option<&str>, c: &str, d: &str) -> Result<Id, PluginError> {
        self.inner.new_row(a, b, c, d)
    }
    fn setting(&self, key: &str) -> Option<serde_json::Value> {
        self.inner.setting(key)
    }
    fn set_setting(&self, key: &str, value: Option<serde_json::Value>) -> Result<(), PluginError> {
        self.inner.set_setting(key, value)
    }
    fn row_data(&self, side_tab: &str, key: &str) -> Option<serde_json::Value> {
        self.inner.row_data(side_tab, key)
    }
    fn set_row_data(
        &self,
        side_tab: &str,
        key: &str,
        value: Option<serde_json::Value>,
    ) -> Result<(), PluginError> {
        self.inner.set_row_data(side_tab, key, value)
    }
    fn set_fact(&self, target: &Target, name: &str, value: Option<serde_json::Value>) {
        self.inner.set_fact(target, name, value)
    }
    fn set_variable(&self, target: &Target, name: &str, value: Option<String>) {
        self.inner.set_variable(target, name, value)
    }
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.inner.emit(event, payload)
    }
    fn agent_log(&self, session: &str) -> Option<std::path::PathBuf> {
        self.inner.agent_log(session)
    }
}

pub struct PluginHost {
    slots: Vec<Arc<Slot>>,
    stopped: Arc<AtomicBool>,
    on_switch: SharedSink,
}

impl PluginHost {
    /// Starts a worker per plugin. `enabled` gives the saved switches; a plugin not in it is on.
    pub fn new(
        plugins: Vec<Arc<dyn Plugin>>,
        cx: CxFactory,
        enabled: &HashMap<String, bool>,
    ) -> Self {
        let stopped = Arc::new(AtomicBool::new(false));
        let slots: Vec<Arc<Slot>> = plugins
            .into_iter()
            .map(|plugin| {
                let manifest = plugin.manifest();
                let on = Arc::new(AtomicBool::new(
                    enabled.get(manifest.id).copied().unwrap_or(true),
                ));
                let paused = Arc::new(AtomicBool::new(false));
                Arc::new(Slot {
                    cx: Arc::new(Guarded {
                        inner: cx(manifest.id),
                        enabled: on.clone(),
                        paused: paused.clone(),
                    }),
                    enabled: on,
                    paused,
                    running: Mutex::new(None),
                    failures: Mutex::new(VecDeque::new()),
                    queue: Arc::new(Queue::default()),
                    plugin,
                    manifest,
                })
            })
            .collect();
        let host = Self {
            slots,
            stopped,
            on_switch: Arc::default(),
        };
        for slot in &host.slots {
            let (slot, stopped, sink) =
                (slot.clone(), host.stopped.clone(), host.on_switch.clone());
            let name = format!("plugin-{}", slot.manifest.id);
            let started = std::thread::Builder::new()
                .name(name)
                .spawn(move || work(&slot, &stopped, &sink));
            if let Err(e) = started {
                eprintln!("terminescu: plugin worker not started: {e}");
            }
        }
        let (slots, stopped, sink) = (
            host.slots.clone(),
            host.stopped.clone(),
            host.on_switch.clone(),
        );
        let started = std::thread::Builder::new()
            .name("plugin-watchdog".into())
            .spawn(move || watchdog(&slots, &stopped, &sink));
        if let Err(e) = started {
            eprintln!("terminescu: plugin watchdog not started: {e}");
        }
        host
    }

    pub fn on_switch(&self, sink: SwitchSink) {
        *lock(&self.on_switch) = Some(sink);
    }

    fn slot(&self, id: &str) -> Option<&Arc<Slot>> {
        self.slots.iter().find(|s| s.manifest.id == id)
    }

    /// Hands `event` to every plugin that is on. Never waits on a plugin.
    pub fn dispatch(&self, event: &CoreEvent) {
        for slot in self.slots.iter().filter(|s| s.live()) {
            let mut q = lock(&slot.queue.events);
            if q.len() >= QUEUE {
                q.pop_front();
            }
            q.push_back(event.clone());
            drop(q);
            slot.queue.ready.notify_one();
        }
    }

    /// A call from a plugin's UI half, under its call budget.
    pub fn call(
        &self,
        plugin: &str,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, PluginError> {
        let slot = self
            .slot(plugin)
            .ok_or_else(|| PluginError::Failed(format!("no plugin {plugin}")))?;
        if !slot.live() {
            return Err(PluginError::Off);
        }
        let (tx, rx) = mpsc::channel();
        let (worker, method_name) = (slot.clone(), method.to_string());
        std::thread::Builder::new()
            .name(format!("plugin-{}-call", slot.manifest.id))
            .spawn(move || {
                let run = catch_unwind(AssertUnwindSafe(|| {
                    worker.plugin.call(&method_name, args, worker.cx.as_ref())
                }));
                let _ = tx.send(run);
            })
            .map_err(|e| PluginError::Failed(e.to_string()))?;
        match rx.recv_timeout(slot.manifest.call_budget) {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.failed(slot, "panicked in a call");
                Err(PluginError::Failed("the plugin failed".into()))
            }
            Err(_) => {
                self.failed(slot, "ran over its call budget");
                Err(PluginError::Timeout)
            }
        }
    }

    fn failed(&self, slot: &Slot, what: &str) {
        if slot.fail(what) {
            self.tell(slot.manifest.id, false);
        }
    }

    fn tell(&self, id: &str, live: bool) {
        tell(&self.on_switch, id, live);
    }

    /// Counts a failure of the agent adapter whose id is `adapter`, as a failure of the plugin
    /// that provides it. The core catches the panic and calls this; the pause follows the same
    /// rule as a failed handler.
    pub fn adapter_failed(&self, adapter: &str, what: &str) {
        let slot = self
            .slots
            .iter()
            .find(|s| s.plugin.agent_adapter().is_some_and(|a| a.id() == adapter));
        if let Some(slot) = slot {
            self.failed(slot, what);
        }
    }

    pub fn list(&self) -> Vec<PluginInfo> {
        self.slots
            .iter()
            .map(|s| PluginInfo {
                id: s.manifest.id.into(),
                name: s.manifest.name.into(),
                description: s.manifest.description.into(),
                enabled: s.enabled.load(Ordering::SeqCst),
                paused: s.paused.load(Ordering::SeqCst),
            })
            .collect()
    }

    /// Switches a plugin. Switching on clears a pause and tells it to rebuild; switching off
    /// drops what it had queued. False for an unknown plugin.
    pub fn set_enabled(&self, id: &str, on: bool) -> bool {
        let Some(slot) = self.slot(id) else {
            return false;
        };
        slot.enabled.store(on, Ordering::SeqCst);
        if on {
            slot.paused.store(false, Ordering::SeqCst);
            lock(&slot.failures).clear();
            let mut q = lock(&slot.queue.events);
            q.push_back(CoreEvent::AppLaunched);
            drop(q);
            slot.queue.ready.notify_one();
        } else {
            lock(&slot.queue.events).clear();
        }
        self.tell(id, on);
        true
    }

    pub fn is_live(&self, id: &str) -> bool {
        self.slot(id).is_some_and(|s| s.live())
    }

    /// The agent adapters of the plugins that are on, in plugin order.
    pub fn adapters(&self) -> Vec<Arc<dyn AgentAdapter>> {
        self.slots
            .iter()
            .filter(|s| s.live())
            .filter_map(|s| s.plugin.agent_adapter())
            .collect()
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        for s in &self.slots {
            s.queue.ready.notify_all();
        }
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A plugin's worker: one event at a time, under the event budget.
fn work(slot: &Slot, stopped: &AtomicBool, sink: &SharedSink) {
    loop {
        let event = {
            let mut q = lock(&slot.queue.events);
            loop {
                if stopped.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(e) = q.pop_front() {
                    break e;
                }
                q = slot
                    .queue
                    .ready
                    .wait_timeout(q, Duration::from_millis(500))
                    .map(|(g, _)| g)
                    .unwrap_or_else(|e| e.into_inner().0);
            }
        };
        if !slot.live() {
            continue;
        }
        let started = Instant::now();
        *lock(&slot.running) = Some(started);
        let run = catch_unwind(AssertUnwindSafe(|| {
            slot.plugin.on_event(&event, slot.cx.as_ref());
        }));
        *lock(&slot.running) = None;
        let paused = if run.is_err() {
            slot.fail("panicked on an event")
        } else if started.elapsed() > slot.manifest.event_budget {
            slot.fail("ran over its event budget")
        } else {
            false
        };
        if paused {
            tell(sink, slot.manifest.id, false);
        }
    }
}

/// Pauses a plugin whose handler has not returned long after its budget: a handler that never
/// returns would otherwise never be counted.
fn watchdog(slots: &[Arc<Slot>], stopped: &AtomicBool, sink: &SharedSink) {
    watchdog_with(slots, stopped, sink, Duration::from_millis(500));
}

fn watchdog_with(slots: &[Arc<Slot>], stopped: &AtomicBool, sink: &SharedSink, poll: Duration) {
    while !stopped.load(Ordering::SeqCst) {
        std::thread::sleep(poll);
        for slot in slots {
            let limit = (slot.manifest.event_budget * HANG_FACTOR).max(HANG_MIN);
            let hung = lock(&slot.running).is_some_and(|t| t.elapsed() > limit);
            if hung && slot.live() && !slot.paused.swap(true, Ordering::SeqCst) {
                eprintln!(
                    "terminescu: plugin {} is stuck in a handler and is paused",
                    slot.manifest.id
                );
                lock(&slot.queue.events).clear();
                tell(sink, slot.manifest.id, false);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::atomic::AtomicUsize;
    use terminescu_plugin_api::{Decoration, Id, PaneInfo, Target};

    /// A facade that does nothing.
    pub(crate) struct NoCx;

    impl PluginCx for NoCx {
        fn panes(&self) -> Vec<PaneInfo> {
            Vec::new()
        }
        fn workspace(&self) -> Value {
            Value::Null
        }
        fn side_tabs(&self, _: &str) -> Vec<Id> {
            Vec::new()
        }
        fn add_context_item(&self, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn add_touched_file(
            &self,
            _: &str,
            _: Option<&str>,
            _: &str,
            _: bool,
        ) -> Result<(), PluginError> {
            Ok(())
        }
        fn set_decoration(&self, _: Decoration) {}
        fn clear_decorations(&self, _: Option<&Target>) {}
        fn open_text(&self, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn type_in_new_tab(&self, _: &str, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn new_row(&self, _: &str, _: Option<&str>, _: &str, _: &str) -> Result<Id, PluginError> {
            Ok(String::new())
        }
        fn setting(&self, _: &str) -> Option<Value> {
            None
        }
        fn set_setting(&self, _: &str, _: Option<Value>) -> Result<(), PluginError> {
            Ok(())
        }
        fn row_data(&self, _: &str, _: &str) -> Option<Value> {
            None
        }
        fn set_row_data(&self, _: &str, _: &str, _: Option<Value>) -> Result<(), PluginError> {
            Ok(())
        }
        fn set_fact(&self, _: &Target, _: &str, _: Option<Value>) {}
        fn set_variable(&self, _: &Target, _: &str, _: Option<String>) {}
        fn emit(&self, _: &str, _: Value) {}
    }

    pub(crate) fn no_cx() -> CxFactory {
        Arc::new(|_| Arc::new(NoCx) as Arc<dyn PluginCx>)
    }

    /// A facade that counts the decorations it is given.
    struct Counting(Arc<AtomicUsize>);

    impl PluginCx for Counting {
        fn panes(&self) -> Vec<PaneInfo> {
            Vec::new()
        }
        fn workspace(&self) -> Value {
            Value::Null
        }
        fn side_tabs(&self, _: &str) -> Vec<Id> {
            Vec::new()
        }
        fn add_context_item(&self, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn add_touched_file(
            &self,
            _: &str,
            _: Option<&str>,
            _: &str,
            _: bool,
        ) -> Result<(), PluginError> {
            Ok(())
        }
        fn set_decoration(&self, _: Decoration) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn clear_decorations(&self, _: Option<&Target>) {}
        fn open_text(&self, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn type_in_new_tab(&self, _: &str, _: &str, _: &str) -> Result<(), PluginError> {
            Ok(())
        }
        fn new_row(&self, _: &str, _: Option<&str>, _: &str, _: &str) -> Result<Id, PluginError> {
            Ok(String::new())
        }
        fn setting(&self, _: &str) -> Option<Value> {
            None
        }
        fn set_setting(&self, _: &str, _: Option<Value>) -> Result<(), PluginError> {
            Ok(())
        }
        fn row_data(&self, _: &str, _: &str) -> Option<Value> {
            None
        }
        fn set_row_data(&self, _: &str, _: &str, _: Option<Value>) -> Result<(), PluginError> {
            Ok(())
        }
        fn set_fact(&self, _: &Target, _: &str, _: Option<Value>) {}
        fn set_variable(&self, _: &Target, _: &str, _: Option<String>) {}
        fn emit(&self, _: &str, _: Value) {}
    }

    fn a_decoration() -> Decoration {
        Decoration {
            target: Target::Pane("p1".into()),
            slot: terminescu_plugin_api::Slot::Chip,
            key: "k".into(),
            text: "t".into(),
            tone: terminescu_plugin_api::Tone::Neutral,
            tip: None,
        }
    }

    #[derive(Clone, Copy)]
    enum Mood {
        Hangs,
        Panics,
        Counts,
    }

    struct Test {
        mood: Mood,
        seen: Arc<AtomicUsize>,
        gate: Arc<(Mutex<bool>, Condvar)>,
    }

    impl Plugin for Test {
        fn manifest(&self) -> Manifest {
            let mut m = Manifest::new("test", "Test", "A plugin for tests");
            if matches!(self.mood, Mood::Hangs) {
                m.event_budget = Duration::from_millis(10);
            }
            m
        }
        fn on_event(&self, _: &CoreEvent, _: &dyn PluginCx) {
            self.seen.fetch_add(1, Ordering::SeqCst);
            match self.mood {
                Mood::Hangs => {
                    let (m, cv) = &*self.gate;
                    let mut open = lock(m);
                    while !*open {
                        open = cv.wait(open).unwrap();
                    }
                }
                Mood::Panics => panic!("a test plugin fails"),
                Mood::Counts => {}
            }
        }
        fn call(&self, method: &str, _: Value, _: &dyn PluginCx) -> Result<Value, PluginError> {
            Ok(Value::from(method))
        }
    }

    type Gate = Arc<(Mutex<bool>, Condvar)>;

    fn host(mood: Mood, on: bool) -> (PluginHost, Arc<AtomicUsize>, Gate) {
        let seen = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let plugin = Test {
            mood,
            seen: seen.clone(),
            gate: gate.clone(),
        };
        let switches = HashMap::from([("test".to_string(), on)]);
        let host = PluginHost::new(vec![Arc::new(plugin)], no_cx(), &switches);
        (host, seen, gate)
    }

    fn until(what: impl Fn() -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn event(i: usize) -> CoreEvent {
        CoreEvent::CwdMoved {
            pane: "p1".into(),
            cwd: format!("/tmp/{i}"),
        }
    }

    #[test]
    fn a_hanging_plugin_does_not_block_patches() {
        let (host, seen, gate) = host(Mood::Hangs, true);
        host.dispatch(&event(0));
        assert!(
            until(|| seen.load(Ordering::SeqCst) == 1),
            "the handler started"
        );
        let started = Instant::now();
        for i in 0..10_000 {
            host.dispatch(&event(i));
        }
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "dispatch never waits"
        );
        assert_eq!(
            lock(&host.slots[0].queue.events).len(),
            QUEUE,
            "the queue stays bounded"
        );
        // Its calls time out instead of waiting on the stuck worker; other work goes on.
        assert_eq!(
            host.call("test", "ping", Value::Null),
            Ok(Value::from("ping"))
        );
        let (m, cv) = &*gate;
        *lock(m) = true;
        cv.notify_all();
        assert!(until(|| lock(&host.slots[0].queue.events).is_empty()));
    }

    #[test]
    fn a_panicking_plugin_is_paused() {
        let (host, seen, _) = host(Mood::Panics, true);
        for i in 0..MAX_FAILURES {
            host.dispatch(&event(i));
        }
        assert!(
            until(|| !host.is_live("test")),
            "paused after five failures"
        );
        assert!(host.list()[0].paused);
        let before = seen.load(Ordering::SeqCst);
        host.dispatch(&event(99));
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            seen.load(Ordering::SeqCst),
            before,
            "a paused plugin hears nothing"
        );
        assert_eq!(
            host.call("test", "ping", Value::Null),
            Err(PluginError::Off)
        );
        // Off and on again clears the pause.
        host.set_enabled("test", false);
        host.set_enabled("test", true);
        assert!(!host.list()[0].paused);
    }

    #[test]
    fn a_disabled_plugin_gets_no_events() {
        let (host, seen, _) = host(Mood::Counts, false);
        host.dispatch(&event(1));
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(seen.load(Ordering::SeqCst), 0);
        assert_eq!(
            host.call("test", "ping", Value::Null),
            Err(PluginError::Off)
        );
        // Switched on, it hears `AppLaunched` and then events.
        assert!(host.set_enabled("test", true));
        host.dispatch(&event(2));
        assert!(until(|| seen.load(Ordering::SeqCst) == 2));
        assert!(!host.set_enabled("nope", true));
    }

    type Told = Arc<Mutex<Vec<(String, bool)>>>;

    fn watched(mood: Mood) -> (PluginHost, Told) {
        let (host, _, _) = host(mood, true);
        let told = Arc::new(Mutex::new(Vec::new()));
        let sink = told.clone();
        host.on_switch(Arc::new(move |id, live| {
            lock(&sink).push((id.to_string(), live));
        }));
        (host, told)
    }

    #[test]
    fn a_pause_from_events_is_announced() {
        let (host, told) = watched(Mood::Panics);
        for i in 0..MAX_FAILURES {
            host.dispatch(&event(i));
        }
        assert!(until(|| !lock(&told).is_empty()), "on_switch fired");
        assert_eq!(lock(&told).as_slice(), [("test".to_string(), false)]);
        assert!(host.list()[0].paused);
    }

    #[test]
    fn a_handler_that_never_returns_is_paused() {
        let (host, told) = watched(Mood::Hangs);
        host.dispatch(&event(0));
        assert!(until(|| !host.is_live("test")), "the watchdog paused it");
        assert!(until(|| !lock(&told).is_empty()));
        assert_eq!(lock(&told)[0], ("test".to_string(), false));
    }

    #[test]
    fn an_adapter_failure_counts_toward_the_pause() {
        struct WithAdapter;
        struct Quiet;
        impl AgentAdapter for Quiet {
            fn id(&self) -> &'static str {
                "quiet"
            }
            fn accepts(&self, _: &Value) -> bool {
                false
            }
            fn on_hook(
                &self,
                a: Option<terminescu_plugin_api::Agent>,
                _: &Value,
            ) -> Option<terminescu_plugin_api::Agent> {
                a
            }
            fn on_line(&self, _: &mut terminescu_plugin_api::Agent, _: &[u8]) -> bool {
                false
            }
            fn events(&self, _: &Value) -> Vec<terminescu_plugin_api::AgentEvent> {
                Vec::new()
            }
            fn resume_command(&self, _: &str) -> Option<String> {
                None
            }
            fn find_transcript(&self, _: &str) -> Option<std::path::PathBuf> {
                None
            }
        }
        impl Plugin for WithAdapter {
            fn manifest(&self) -> Manifest {
                Manifest::new("quiet", "Quiet", "An adapter")
            }
            fn on_event(&self, _: &CoreEvent, _: &dyn PluginCx) {}
            fn call(&self, _: &str, _: Value, _: &dyn PluginCx) -> Result<Value, PluginError> {
                Ok(Value::Null)
            }
            fn agent_adapter(&self) -> Option<Arc<dyn AgentAdapter>> {
                Some(Arc::new(Quiet))
            }
        }
        let host = PluginHost::new(vec![Arc::new(WithAdapter)], no_cx(), &HashMap::new());
        let told = Arc::new(Mutex::new(Vec::new()));
        let sink = told.clone();
        host.on_switch(Arc::new(move |id, live| {
            lock(&sink).push((id.to_string(), live))
        }));
        for _ in 0..MAX_FAILURES {
            host.adapter_failed("quiet", "panicked in on_hook");
        }
        assert!(!host.is_live("quiet"));
        assert_eq!(lock(&told).as_slice(), [("quiet".to_string(), false)]);
        host.adapter_failed("other", "ignored");
    }

    #[test]
    fn a_plugin_that_is_off_cannot_add_decorations() {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let cx: CxFactory = Arc::new(move |_| Arc::new(Counting(c.clone())) as Arc<dyn PluginCx>);
        let seen = Arc::new(AtomicUsize::new(0));
        let plugin = Test {
            mood: Mood::Counts,
            seen,
            gate: Arc::new((Mutex::new(false), Condvar::new())),
        };
        let host = PluginHost::new(vec![Arc::new(plugin)], cx, &HashMap::new());
        let facade = host.slots[0].cx.clone();
        facade.set_decoration(a_decoration());
        assert_eq!(count.load(Ordering::SeqCst), 1, "a live plugin decorates");
        host.set_enabled("test", false);
        // A handler that was already running calls in after the switch.
        facade.set_decoration(a_decoration());
        assert_eq!(count.load(Ordering::SeqCst), 1, "an off plugin does not");
        host.set_enabled("test", true);
        facade.set_decoration(a_decoration());
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
