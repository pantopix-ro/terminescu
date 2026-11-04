//! Terminal views: pty output on its way to the windows (D33).
//!
//! A view is one window showing one tab's pane. Output queues per view and leaves in batches of
//! at most [`MAX_BATCH`], one per frame. The window acknowledges each batch once xterm.js has
//! written it. Past [`ACK_WINDOW`] bytes received but not acknowledged, the view goes stale: its
//! queue is dropped and nothing more is forwarded. Once the window has acknowledged everything
//! already sent, the view attaches again and the daemon's snapshot redraws it from scratch.
//!
//! This module holds the bookkeeping only; [`crate::runtime`] moves the bytes.

use std::collections::HashMap;
use std::sync::Arc;

/// Largest batch sent in one message.
pub const MAX_BATCH: usize = 256 * 1024;
/// Bytes a view may have queued or in flight before it goes stale.
pub const ACK_WINDOW: u64 = 1024 * 1024;

/// Delivers one batch to a window. Returns false once the window is gone.
pub type Sink = Arc<dyn Fn(Vec<u8>) -> bool + Send + Sync>;

pub type ViewId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewState {
    /// Waiting for the daemon's `Attached`; output before it belongs to an older stream.
    Attaching,
    Live,
    /// Fell behind; waits for the window to acknowledge what is in flight.
    Stale,
}

struct View {
    id: ViewId,
    sink: Sink,
    cols: u16,
    rows: u16,
    state: ViewState,
    queue: Vec<u8>,
    sent: u64,
    acked: u64,
}

impl View {
    fn in_flight(&self) -> u64 {
        self.sent.saturating_sub(self.acked)
    }
}

/// What the runtime must tell the daemon after a hub call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Resize the tab's pane, then attach to it.
    Attach { tab: String, cols: u16, rows: u16 },
}

/// The views of every tab, keyed by tab id. A tab has at most one view; opening another
/// replaces it, so a window that reloads takes over its tab.
#[derive(Default)]
pub struct Views {
    by_tab: HashMap<String, View>,
    next: ViewId,
}

impl Views {
    /// Registers a view and returns its id with the attach the runtime should send.
    pub fn open(&mut self, tab: &str, cols: u16, rows: u16, sink: Sink) -> (ViewId, Action) {
        self.next += 1;
        let id = self.next;
        self.by_tab.insert(
            tab.into(),
            View {
                id,
                sink,
                cols,
                rows,
                state: ViewState::Attaching,
                queue: Vec::new(),
                sent: 0,
                acked: 0,
            },
        );
        (
            id,
            Action::Attach {
                tab: tab.into(),
                cols,
                rows,
            },
        )
    }

    /// Removes a view. Returns its tab when the view was still current, so the runtime can
    /// detach the pane.
    pub fn close(&mut self, view: ViewId) -> Option<String> {
        let tab = self.tab_of(view)?;
        self.by_tab.remove(&tab);
        Some(tab)
    }

    pub fn tab_of(&self, view: ViewId) -> Option<String> {
        self.by_tab
            .iter()
            .find(|(_, v)| v.id == view)
            .map(|(t, _)| t.clone())
    }

    pub fn state(&self, tab: &str) -> Option<ViewState> {
        self.by_tab.get(tab).map(|v| v.state)
    }

    pub fn has(&self, tab: &str) -> bool {
        self.by_tab.contains_key(tab)
    }

    /// Drops a closed tab's view; its window unmounts it on the same patch.
    pub fn remove_tab(&mut self, tab: &str) {
        self.by_tab.remove(tab);
    }

    /// Records a new size. Returns the view's tab, or None when the view is not current.
    pub fn resize(&mut self, view: ViewId, cols: u16, rows: u16) -> Option<String> {
        let tab = self.tab_of(view)?;
        let v = self.by_tab.get_mut(&tab)?;
        v.cols = cols;
        v.rows = rows;
        Some(tab)
    }

    /// The attach for a tab whose pane just changed (spawned, or a new daemon connection).
    pub fn reattach(&mut self, tab: &str) -> Option<Action> {
        let v = self.by_tab.get_mut(tab)?;
        v.state = ViewState::Attaching;
        v.queue.clear();
        Some(Action::Attach {
            tab: tab.into(),
            cols: v.cols,
            rows: v.rows,
        })
    }

    /// Every view's attach, after a reconnect.
    pub fn reattach_all(&mut self) -> Vec<Action> {
        let tabs: Vec<String> = self.by_tab.keys().cloned().collect();
        tabs.iter().filter_map(|t| self.reattach(t)).collect()
    }

    /// The daemon confirmed an attach: what follows is a fresh snapshot.
    pub fn attached(&mut self, tab: &str) {
        if let Some(v) = self.by_tab.get_mut(tab)
            && v.state == ViewState::Attaching
        {
            v.state = ViewState::Live;
            v.queue.clear();
        }
    }

    /// Queues output. Returns true when the view has bytes ready to send.
    pub fn output(&mut self, tab: &str, data: &[u8]) -> bool {
        let Some(v) = self.by_tab.get_mut(tab) else {
            return false;
        };
        if v.state != ViewState::Live {
            return false;
        }
        if v.in_flight() + (v.queue.len() + data.len()) as u64 > ACK_WINDOW {
            v.state = ViewState::Stale;
            v.queue = Vec::new();
            return false;
        }
        v.queue.extend_from_slice(data);
        true
    }

    /// Takes one batch from every view with queued output.
    pub fn take_batches(&mut self) -> Vec<(ViewId, Sink, Vec<u8>)> {
        let mut out = Vec::new();
        for v in self.by_tab.values_mut() {
            if v.state != ViewState::Live || v.queue.is_empty() {
                continue;
            }
            let n = v.queue.len().min(MAX_BATCH);
            let rest = v.queue.split_off(n);
            let batch = std::mem::replace(&mut v.queue, rest);
            v.sent += n as u64;
            out.push((v.id, v.sink.clone(), batch));
        }
        out
    }

    pub fn has_queued(&self) -> bool {
        self.by_tab
            .values()
            .any(|v| v.state == ViewState::Live && !v.queue.is_empty())
    }

    /// The window wrote `bytes`. A stale view whose flight has landed attaches again.
    pub fn ack(&mut self, view: ViewId, bytes: u64) -> Option<Action> {
        let tab = self.tab_of(view)?;
        let v = self.by_tab.get_mut(&tab)?;
        v.acked = (v.acked + bytes).min(v.sent);
        if v.state == ViewState::Stale && v.in_flight() == 0 {
            return self.reattach(&tab);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn sink() -> (Sink, Arc<Mutex<Vec<Vec<u8>>>>) {
        let got = Arc::new(Mutex::new(Vec::new()));
        let g = got.clone();
        (
            Arc::new(move |b| {
                g.lock().unwrap().push(b);
                true
            }),
            got,
        )
    }

    fn sent(views: &mut Views) -> Vec<usize> {
        views
            .take_batches()
            .into_iter()
            .map(|(_, s, b)| {
                let n = b.len();
                s(b);
                n
            })
            .collect()
    }

    #[test]
    fn output_before_attached_is_dropped() {
        let mut views = Views::default();
        let (s, _) = sink();
        let (_, action) = views.open("t1", 100, 30, s);
        assert_eq!(
            action,
            Action::Attach {
                tab: "t1".into(),
                cols: 100,
                rows: 30
            }
        );
        assert!(!views.output("t1", b"old"));
        views.attached("t1");
        assert!(views.output("t1", b"new"));
        let batches = views.take_batches();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].2, b"new");
    }

    #[test]
    fn batches_stay_under_the_cap() {
        let mut views = Views::default();
        let (s, _) = sink();
        let (id, _) = views.open("t1", 80, 24, s);
        views.attached("t1");
        views.output("t1", &vec![b'x'; MAX_BATCH + 10]);
        assert_eq!(sent(&mut views), vec![MAX_BATCH]);
        assert_eq!(sent(&mut views), vec![10]);
        assert!(sent(&mut views).is_empty());
        assert_eq!(views.ack(id, (MAX_BATCH + 10) as u64), None);
    }

    #[test]
    fn falling_behind_goes_stale_and_resyncs_after_the_flight_lands() {
        let mut views = Views::default();
        let (s, _) = sink();
        let (id, _) = views.open("t1", 80, 24, s);
        views.attached("t1");
        let chunk = vec![b'x'; 200 * 1024];
        for _ in 0..4 {
            assert!(views.output("t1", &chunk));
            sent(&mut views);
        }
        // 800 KB in flight; 300 KB more crosses the window.
        assert!(!views.output("t1", &vec![b'y'; 300 * 1024]));
        assert_eq!(views.state("t1"), Some(ViewState::Stale));
        assert!(!views.output("t1", b"z"));
        assert!(sent(&mut views).is_empty());
        assert_eq!(views.ack(id, 400 * 1024), None);
        assert_eq!(
            views.ack(id, 400 * 1024),
            Some(Action::Attach {
                tab: "t1".into(),
                cols: 80,
                rows: 24
            })
        );
        assert_eq!(views.state("t1"), Some(ViewState::Attaching));
        views.attached("t1");
        assert!(views.output("t1", b"fresh"));
    }

    #[test]
    fn a_second_view_replaces_the_first() {
        let mut views = Views::default();
        let (a, _) = sink();
        let (b, _) = sink();
        let (first, _) = views.open("t1", 80, 24, a);
        let (second, _) = views.open("t1", 90, 20, b);
        assert_ne!(first, second);
        assert_eq!(
            views.close(first),
            None,
            "a replaced view closes as a no-op"
        );
        assert!(views.has("t1"));
        assert_eq!(views.resize(second, 120, 40), Some("t1".into()));
        assert_eq!(
            views.reattach_all(),
            vec![Action::Attach {
                tab: "t1".into(),
                cols: 120,
                rows: 40
            }]
        );
        assert_eq!(views.close(second), Some("t1".into()));
        assert!(!views.has("t1"));
    }
}
