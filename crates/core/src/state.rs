//! The workspace model and its command and patch protocol (D6).
//!
//! A window sends a [`Command`]; the core checks it against the workspace, turns it into a
//! [`PatchBatch`], applies the batch to its own copy and broadcasts it to every window. Each
//! window applies the same patches to its store, so all copies stay equal. `rev` numbers the
//! batches; a window that sees a gap asks for a [`Snapshot`].

use crate::layout::{Dir, Layout, LayoutError};
use crate::status::ManualStatus;
use serde::{Deserialize, Serialize};

pub type Id = String;

/// The icons a side tab or tab can pick instead of its default one (task 8.16). Each name is
/// an `ao-i-*` class of the design system.
pub const ICONS: &[&str] = &[
    "term", "agent", "folder", "tab", "window", "split", "doc", "tpl", "prompt", "var", "link",
    "pin", "play", "check", "bp", "pr", "merge", "save", "in", "out",
];

/// The icon a new tab opens with (task 8.20): one no other tab uses yet, picked from the tab
/// id so it looks random but replays the same in tests. The agent icon stays for Claude and
/// the folder icon for folders.
/// When every icon is taken it reuses one.
pub fn pick_icon(ws: &Workspace, tab_id: &str, also: &[Option<String>]) -> String {
    let pool: Vec<&str> = ICONS
        .iter()
        .copied()
        .filter(|i| !matches!(*i, "agent" | "folder"))
        .collect();
    let used: Vec<&str> = ws
        .windows
        .iter()
        .flat_map(|w| &w.side_tabs)
        .flat_map(|s| &s.tabs)
        .filter_map(|t| t.icon.as_deref())
        .chain(also.iter().filter_map(|i| i.as_deref()))
        .collect();
    pick_unused(&pool, tab_id, &used)
}

/// The animals and birds a group is named after (tasks 8.30, 8.60). Short, plain words, so a name
/// fits a tab and reads out loud.
pub const NAMES: &[&str] = &[
    "Otter", "Heron", "Lynx", "Wren", "Ibis", "Fox", "Badger", "Marten", "Stoat", "Hare", "Bison",
    "Moose", "Elk", "Ibex", "Yak", "Llama", "Alpaca", "Tapir", "Okapi", "Panda", "Koala", "Lemur",
    "Gecko", "Iguana", "Newt", "Toad", "Seal", "Walrus", "Orca", "Dolphin", "Narwhal", "Beaver",
    "Mole", "Shrew", "Vole", "Hedgehog", "Puma", "Jaguar", "Ocelot", "Cheetah", "Zebra", "Gazelle",
    "Impala", "Camel", "Dingo", "Coyote", "Wolf", "Bear", "Raccoon", "Ferret", "Mink", "Sloth",
    "Wombat", "Quokka", "Meerkat", "Mongoose", "Gibbon", "Tiger", "Leopard", "Robin", "Finch",
    "Lark", "Swift", "Crane", "Stork", "Egret", "Kestrel", "Falcon", "Osprey", "Owl", "Raven",
    "Magpie", "Jay", "Puffin", "Gannet", "Tern", "Plover", "Kiwi", "Toucan", "Parrot", "Macaw",
    "Pelican", "Condor", "Sparrow", "Starling", "Swallow", "Oriole", "Hoopoe", "Grouse", "Quail",
    "Dove", "Curlew", "Snipe", "Avocet",
];

/// A new side tab's name (task 8.60): an animal or bird no side tab in any window has yet,
/// picked from its id so it looks random but replays the same in tests. The sidebar shows it
/// once the side tab holds two tabs or more. Once every name is taken, a number follows the
/// picked one: "Otter 2".
pub fn pick_name(ws: &Workspace, side_tab_id: &str) -> String {
    let used: Vec<&str> = ws
        .windows
        .iter()
        .flat_map(|w| &w.side_tabs)
        .filter_map(|s| s.name.as_deref())
        .collect();
    let name = pick_unused(NAMES, side_tab_id, &used);
    if !used.contains(&name.as_str()) {
        return name;
    }
    (2..)
        .map(|n| format!("{name} {n}"))
        .find(|n| !used.contains(&n.as_str()))
        .unwrap_or(name)
}

/// The neon accents a sidebar row can take (task 8.25), as `--neon-*` tokens. No amber,
/// orange or yellow: amber means "needs you".
pub const NEONS: &[&str] = &[
    "cyan", "pink", "lime", "violet", "mint", "blue", "rose", "ice",
];

/// A reopened tab's ids are all free: neither the tab nor any of its panes came back already.
fn absent(ws: &Workspace, tab: &Tab) -> bool {
    ws.tab(&tab.id).is_none()
        && tab
            .layout
            .panes()
            .iter()
            .all(|p| ws.locate_pane(p).is_none())
}

/// The neon a new side tab opens with: one no other row in the workspace uses yet, picked from
/// its id like [`pick_icon`]. When every neon is taken it reuses one.
pub fn pick_neon(ws: &Workspace, side_tab_id: &str) -> String {
    let used: Vec<&str> = ws
        .windows
        .iter()
        .flat_map(|w| &w.side_tabs)
        .filter_map(|s| s.accent.as_deref())
        .collect();
    pick_unused(NEONS, side_tab_id, &used)
}

/// The neon a new tab opens with (task 8.26): one its side tab's other tabs don't use yet.
pub fn pick_tab_neon(siblings: &[Tab], tab_id: &str) -> String {
    let used: Vec<&str> = siblings
        .iter()
        .filter_map(|t| t.accent.as_deref())
        .collect();
    pick_unused(NEONS, tab_id, &used)
}

/// Gives a neon to every side tab and tab saved before neons existed, as a new one would get,
/// an icon to every tab saved before tabs got one (task 8.29) and a name to every side tab
/// saved before side tabs got one (task 8.60).
pub fn fill_accents(ws: &mut Workspace) {
    let unnamed: Vec<(usize, usize)> = ws
        .windows
        .iter()
        .enumerate()
        .flat_map(|(w, win)| {
            win.side_tabs
                .iter()
                .enumerate()
                .filter(|(_, s)| s.name.is_none())
                .map(move |(s, _)| (w, s))
        })
        .collect();
    for (w, s) in unnamed {
        let id = ws.windows[w].side_tabs[s].id.clone();
        ws.windows[w].side_tabs[s].name = Some(pick_name(ws, &id));
    }
    let bare: Vec<(usize, usize, usize)> = ws
        .windows
        .iter()
        .enumerate()
        .flat_map(|(w, win)| {
            win.side_tabs.iter().enumerate().flat_map(move |(s, side)| {
                side.tabs
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.icon.is_none())
                    .map(move |(t, _)| (w, s, t))
            })
        })
        .collect();
    for (w, s, t) in bare {
        let id = ws.windows[w].side_tabs[s].tabs[t].id.clone();
        ws.windows[w].side_tabs[s].tabs[t].icon = Some(pick_icon(ws, &id, &[]));
    }
    for w in 0..ws.windows.len() {
        for s in 0..ws.windows[w].side_tabs.len() {
            if ws.windows[w].side_tabs[s].accent.is_none() {
                let id = ws.windows[w].side_tabs[s].id.clone();
                ws.windows[w].side_tabs[s].accent = Some(pick_neon(ws, &id));
            }
            let side = &mut ws.windows[w].side_tabs[s];
            for t in 0..side.tabs.len() {
                if side.tabs[t].accent.is_none() {
                    side.tabs[t].accent = Some(pick_tab_neon(&side.tabs, &side.tabs[t].id));
                }
            }
        }
    }
}

/// The first entry of `pool` not in `used`, starting at a hash of `seed`.
fn pick_unused(pool: &[&str], seed: &str, used: &[&str]) -> String {
    // FNV-1a: stable across builds, unlike the std hasher.
    let hash = seed.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    let start = (hash % pool.len() as u64) as usize;
    (0..pool.len())
        .map(|k| pool[(start + k) % pool.len()])
        .find(|i| !used.contains(i))
        .unwrap_or(pool[start])
        .to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub windows: Vec<Window>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub id: Id,
    pub side_tabs: Vec<SideTab>,
}

/// A sidebar row. It holds one or more tabs and owns a context (D3, D4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideTab {
    pub id: Id,
    pub title: Option<String>,
    pub tabs: Vec<Tab>,
    /// Set by hand in the status editor; absent means the generated status shows (D15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ManualStatus>,
    /// One of [`ICONS`]; absent shows the default icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// One of [`NEONS`], given when the row is made (task 8.25); absent keeps the neutral edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// An animal or bird, given when the row is made (task 8.60). It names the row while the row
    /// holds two tabs or more; `title` overrides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Made as a folder from the sidebar's + (task 8.91): it stays a folder with one tab or
    /// none, where other rows close with their last tab.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub folder: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: Id,
    pub title: Option<String>,
    /// The tab's split tree; its leaves are panes (D3).
    pub layout: Layout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ManualStatus>,
    /// One of [`ICONS`]; absent shows the default icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// One of [`NEONS`] (task 8.26): the icon's colour and the tab's square in the sidebar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

/// The side of a pane a dropped tab lands on (task 8.32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockSide {
    Left,
    Right,
    Up,
    Down,
}

impl DockSide {
    /// The split direction, and whether the tab goes before the pane.
    pub fn place(self) -> (Dir, bool) {
        match self {
            DockSide::Left => (Dir::Row, true),
            DockSide::Right => (Dir::Row, false),
            DockSide::Up => (Dir::Column, true),
            DockSide::Down => (Dir::Column, false),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Command {
    /// A new window holding one new side tab with one new tab (task 8.81).
    CreateWindow,
    /// A new side tab with one new tab, at `index` in the window's sidebar (end when absent).
    CreateSideTab {
        window_id: Id,
        index: Option<usize>,
    },
    /// An empty folder at `index` in the window's sidebar (end when absent), task 8.91.
    CreateFolder {
        window_id: Id,
        index: Option<usize>,
    },
    /// A new tab at the end of a side tab.
    CreateTab {
        side_tab_id: Id,
    },
    CloseTab {
        tab_id: Id,
    },
    /// Reorder inside a side tab or move into another one. `index` is the position in the
    /// target after the tab left its old place.
    MoveTab {
        tab_id: Id,
        to_side_tab_id: Id,
        index: usize,
    },
    /// Drop on a sidebar gap: a new side tab at `index` holding the tab (D5). The only tab of
    /// a side tab moves its side tab instead, as [`Command::MoveSideTab`], so its context stays.
    MoveTabToNewSideTab {
        tab_id: Id,
        window_id: Id,
        index: usize,
    },
    /// Drop outside every window (D20): a new window with one new side tab holding the tab. The
    /// only tab of a side tab takes its side tab along, as [`Command::MoveSideTabToNewWindow`].
    MoveTabToNewWindow {
        tab_id: Id,
    },
    RenameTab {
        tab_id: Id,
        title: Option<String>,
    },
    /// A side tab's own name; `None` or blank goes back to its first tab's name (task 8.7).
    RenameSideTab {
        side_tab_id: Id,
        title: Option<String>,
    },
    /// A side tab's or tab's icon from [`ICONS`]; `None` goes back to the default (task 8.16).
    SetIcon {
        target_id: Id,
        icon: Option<String>,
    },
    /// A side tab's or tab's neon from [`NEONS`], picked in the edit dialog (task 8.34).
    SetAccent {
        target_id: Id,
        accent: String,
    },
    /// Closes a side tab with every tab in it; its context goes with it (task 8.7).
    CloseSideTab {
        side_tab_id: Id,
    },
    /// A new window holding the side tab, its tabs and its context (task 8.7).
    MoveSideTabToNewWindow {
        side_tab_id: Id,
    },
    /// A side tab, its tabs and its context to the gap before side tab `index` of a window,
    /// counted before it leaves its place; its own window or another (task 8.70).
    MoveSideTab {
        side_tab_id: Id,
        window_id: Id,
        index: usize,
    },
    /// Every other window's side tabs to the end of `window_id`, in window order, as side tabs;
    /// the emptied windows close (task 8.84).
    MergeAllWindows {
        window_id: Id,
    },
    /// Drop a side tab onto another (D5): its tabs move to the end of `into` and it closes.
    /// The contexts merge outside the workspace, in [`crate::merge`].
    MergeSideTab {
        from: Id,
        into: Id,
    },
    /// Undo of a merge: `side_tab` (its tabs left empty) comes back at `index` in the window
    /// and `tab_ids` move back into it, in order.
    RestoreSideTab {
        window_id: Id,
        index: usize,
        side_tab: SideTab,
        tab_ids: Vec<Id>,
    },
    /// Undo close (task 8.87): a closed row comes back at `index` in the window, ids and all,
    /// so its saved context finds it again.
    ReopenSideTab {
        window_id: Id,
        index: usize,
        side_tab: SideTab,
    },
    /// Undo close: a closed tab comes back at `index` in its row or, when that row is gone, as
    /// a new row at the end of `window_id`.
    ReopenTab {
        window_id: Id,
        side_tab_id: Id,
        index: usize,
        tab: Tab,
    },
    /// A new pane beside `pane_id`: right for `row`, below for `column`.
    SplitPane {
        pane_id: Id,
        dir: Dir,
    },
    /// A tab dropped on a side of a pane (task 8.32): its panes join the pane's tab on that
    /// side, keeping their shells, and the dropped tab closes.
    DockTab {
        tab_id: Id,
        pane_id: Id,
        side: DockSide,
    },
    /// A pane dragged out to the tab bar (task 8.32): it leaves its tab and becomes a new tab
    /// at `index` in the same side tab, keeping its shell.
    PaneToTab {
        pane_id: Id,
        index: usize,
    },
    /// A read-only transcript of `session` beside `pane_id`, on the right (task 4.4).
    OpenTranscript {
        pane_id: Id,
        session: String,
    },
    /// A text pane editing `doc_id` (D27, D28): beside `beside` (a split to the right), or in
    /// a new tab at the end of the side tab.
    OpenText {
        side_tab_id: Id,
        doc_id: Id,
        beside: Option<Id>,
    },
    /// Closes a pane; the last pane of a tab closes the tab.
    ClosePane {
        pane_id: Id,
    },
    /// New sizes for a split, in any unit; they scale to thousandths.
    ResizeSplit {
        split_id: Id,
        sizes: Vec<u16>,
    },
    /// Types `claude --resume <id>` and Enter into a pane that offers it (D23).
    ResumePane {
        pane_id: Id,
    },
    /// Drops a pane's resume offer without running anything.
    DismissResume {
        pane_id: Id,
    },
    /// Sets the manual status of a tab or side tab (`target_id`), or goes back to auto with
    /// `None` (D15).
    SetStatus {
        target_id: Id,
        status: Option<ManualStatus>,
    },
    /// A saved status changed: every tab and side tab using it takes the new look, or goes
    /// back to auto when it was deleted (`None`).
    RestyleStatus {
        status_id: Id,
        status: Option<ManualStatus>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Patch {
    /// An empty window; the side tab it gets comes in the same batch.
    WindowCreated {
        window_id: Id,
    },
    /// A window that lost its last side tab, while another window remains (task 8.1).
    WindowClosed {
        window_id: Id,
    },
    SideTabCreated {
        window_id: Id,
        index: usize,
        side_tab: SideTab,
    },
    SideTabClosed {
        side_tab_id: Id,
    },
    SideTabRenamed {
        side_tab_id: Id,
        title: Option<String>,
    },
    TabCreated {
        side_tab_id: Id,
        index: usize,
        tab: Tab,
    },
    TabClosed {
        tab_id: Id,
    },
    TabMoved {
        tab_id: Id,
        to_side_tab_id: Id,
        index: usize,
    },
    TabRenamed {
        tab_id: Id,
        title: Option<String>,
    },
    IconSet {
        id: Id,
        icon: Option<String>,
    },
    AccentSet {
        id: Id,
        accent: String,
    },
    /// The tab's whole split tree after a split, close, resize or resume change.
    LayoutChanged {
        tab_id: Id,
        layout: Layout,
    },
    /// A tab's or side tab's manual status (`id` names either).
    StatusSet {
        id: Id,
        status: Option<ManualStatus>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchBatch {
    pub rev: u64,
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub rev: u64,
    pub workspace: Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", content = "id")]
pub enum CommandError {
    #[error("unknown window {0}")]
    UnknownWindow(Id),
    #[error("unknown side tab {0}")]
    UnknownSideTab(Id),
    #[error("unknown tab {0}")]
    UnknownTab(Id),
    #[error("index {0} is out of range")]
    IndexOutOfRange(usize),
    #[error("unknown pane {0}")]
    UnknownPane(Id),
    #[error("unknown split {0}")]
    UnknownSplit(Id),
    #[error("sizes do not fit split {0}")]
    BadSizes(Id),
    #[error("pane {0} has no session to resume")]
    NothingToResume(Id),
    #[error("pane {0} is too small to split")]
    PaneTooSmall(Id),
    #[error("{0} is not a Claude session id")]
    BadSession(String),
    #[error("{0}")]
    BadStatus(String),
    #[error("{0} is not a known icon")]
    BadIcon(String),
    #[error("{0} is not a known colour")]
    BadAccent(String),
    #[error("tab {0} cannot dock beside its own pane")]
    DockOntoItself(Id),
    #[error("pane {0} is its tab's only pane")]
    OnlyPane(Id),
}

impl Workspace {
    fn side_tab_mut(&mut self, id: &str) -> Option<&mut SideTab> {
        self.windows
            .iter_mut()
            .flat_map(|w| w.side_tabs.iter_mut())
            .find(|s| s.id == id)
    }

    pub fn side_tab(&self, id: &str) -> Option<&SideTab> {
        self.windows
            .iter()
            .flat_map(|w| w.side_tabs.iter())
            .find(|s| s.id == id)
    }

    /// The side tab holding a tab, and the tab's index in it.
    pub fn locate_tab(&self, tab_id: &str) -> Option<(&SideTab, usize)> {
        self.windows
            .iter()
            .flat_map(|w| w.side_tabs.iter())
            .find_map(|s| s.tabs.iter().position(|t| t.id == tab_id).map(|i| (s, i)))
    }

    pub fn tab(&self, tab_id: &str) -> Option<&Tab> {
        self.locate_tab(tab_id).map(|(s, i)| &s.tabs[i])
    }

    fn tab_mut(&mut self, tab_id: &str) -> Option<&mut Tab> {
        self.windows
            .iter_mut()
            .flat_map(|w| w.side_tabs.iter_mut())
            .flat_map(|s| s.tabs.iter_mut())
            .find(|t| t.id == tab_id)
    }

    /// The side tab and tab holding a pane.
    pub fn locate_pane(&self, pane_id: &str) -> Option<(&SideTab, &Tab)> {
        self.windows
            .iter()
            .flat_map(|w| w.side_tabs.iter())
            .find_map(|s| {
                s.tabs
                    .iter()
                    .find(|t| t.layout.has_pane(pane_id))
                    .map(|t| (s, t))
            })
    }

    /// Every pane that runs a shell with its side tab, in order: transcript panes left out.
    pub fn terminal_sides(&self) -> Vec<(Id, Id)> {
        self.windows
            .iter()
            .flat_map(|w| w.side_tabs.iter())
            .flat_map(|s| {
                s.tabs
                    .iter()
                    .flat_map(|t| t.layout.terminals())
                    .map(|p| (p.clone(), s.id.clone()))
            })
            .collect()
    }

    /// Every text pane as (pane, document, side tab), in order (D28).
    pub fn doc_panes(&self) -> Vec<(Id, Id, Id)> {
        self.windows
            .iter()
            .flat_map(|w| w.side_tabs.iter())
            .flat_map(|s| {
                s.tabs
                    .iter()
                    .flat_map(|t| t.layout.docs())
                    .map(|(p, d)| (p.clone(), d.clone(), s.id.clone()))
            })
            .collect()
    }

    /// Every pane with its side tab, in order.
    pub fn pane_sides(&self) -> Vec<(Id, Id)> {
        self.windows
            .iter()
            .flat_map(|w| w.side_tabs.iter())
            .flat_map(|s| {
                s.tabs
                    .iter()
                    .flat_map(|t| t.layout.panes())
                    .map(|p| (p.clone(), s.id.clone()))
            })
            .collect()
    }

    fn take_tab(&mut self, tab_id: &str) -> Option<Tab> {
        for s in self.windows.iter_mut().flat_map(|w| w.side_tabs.iter_mut()) {
            if let Some(i) = s.tabs.iter().position(|t| t.id == tab_id) {
                return Some(s.tabs.remove(i));
            }
        }
        None
    }
}

/// Applies one patch. Patches come from the core, which checked them; a patch that does not
/// fit means the copy is out of date, and the caller resyncs from a snapshot.
pub fn apply(ws: &mut Workspace, patch: &Patch) -> Result<(), CommandError> {
    match patch {
        Patch::WindowCreated { window_id } => {
            if ws.windows.iter().any(|w| &w.id == window_id) {
                return Err(CommandError::UnknownWindow(window_id.clone()));
            }
            ws.windows.push(Window {
                id: window_id.clone(),
                side_tabs: vec![],
            });
        }
        Patch::WindowClosed { window_id } => {
            // Only an empty window closes: its tabs must have moved first.
            let i = ws
                .windows
                .iter()
                .position(|w| &w.id == window_id && w.side_tabs.is_empty())
                .ok_or_else(|| CommandError::UnknownWindow(window_id.clone()))?;
            ws.windows.remove(i);
        }
        Patch::SideTabCreated {
            window_id,
            index,
            side_tab,
        } => {
            let w = ws
                .windows
                .iter_mut()
                .find(|w| &w.id == window_id)
                .ok_or_else(|| CommandError::UnknownWindow(window_id.clone()))?;
            if *index > w.side_tabs.len() {
                return Err(CommandError::IndexOutOfRange(*index));
            }
            w.side_tabs.insert(*index, side_tab.clone());
        }
        Patch::SideTabClosed { side_tab_id } => {
            let w = ws
                .windows
                .iter_mut()
                .find(|w| w.side_tabs.iter().any(|s| &s.id == side_tab_id))
                .ok_or_else(|| CommandError::UnknownSideTab(side_tab_id.clone()))?;
            w.side_tabs.retain(|s| &s.id != side_tab_id);
        }
        Patch::TabCreated {
            side_tab_id,
            index,
            tab,
        } => {
            let s = ws
                .side_tab_mut(side_tab_id)
                .ok_or_else(|| CommandError::UnknownSideTab(side_tab_id.clone()))?;
            if *index > s.tabs.len() {
                return Err(CommandError::IndexOutOfRange(*index));
            }
            s.tabs.insert(*index, tab.clone());
        }
        Patch::TabClosed { tab_id } => {
            ws.take_tab(tab_id)
                .ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
        }
        Patch::TabMoved {
            tab_id,
            to_side_tab_id,
            index,
        } => {
            let len = ws
                .side_tab(to_side_tab_id)
                .ok_or_else(|| CommandError::UnknownSideTab(to_side_tab_id.clone()))?
                .tabs
                .iter()
                .filter(|t| &t.id != tab_id)
                .count();
            if *index > len {
                return Err(CommandError::IndexOutOfRange(*index));
            }
            let tab = ws
                .take_tab(tab_id)
                .ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
            ws.side_tab_mut(to_side_tab_id)
                .expect("checked above")
                .tabs
                .insert(*index, tab);
        }
        Patch::SideTabRenamed { side_tab_id, title } => {
            let s = ws
                .side_tab_mut(side_tab_id)
                .ok_or_else(|| CommandError::UnknownSideTab(side_tab_id.clone()))?;
            s.title = title.clone();
        }
        Patch::TabRenamed { tab_id, title } => {
            let tab = ws
                .tab_mut(tab_id)
                .ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
            tab.title = title.clone();
        }
        Patch::IconSet { id, icon } => {
            if let Some(tab) = ws.tab_mut(id) {
                tab.icon = icon.clone();
            } else {
                ws.side_tab_mut(id)
                    .ok_or_else(|| CommandError::UnknownTab(id.clone()))?
                    .icon = icon.clone();
            }
        }
        Patch::AccentSet { id, accent } => {
            if let Some(tab) = ws.tab_mut(id) {
                tab.accent = Some(accent.clone());
            } else {
                ws.side_tab_mut(id)
                    .ok_or_else(|| CommandError::UnknownTab(id.clone()))?
                    .accent = Some(accent.clone());
            }
        }
        Patch::LayoutChanged { tab_id, layout } => {
            let tab = ws
                .tab_mut(tab_id)
                .ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
            tab.layout = layout.clone();
        }
        Patch::StatusSet { id, status } => {
            if let Some(tab) = ws.tab_mut(id) {
                tab.status = status.clone();
            } else {
                ws.side_tab_mut(id)
                    .ok_or_else(|| CommandError::UnknownTab(id.clone()))?
                    .status = status.clone();
            }
        }
    }
    Ok(())
}

pub struct Core {
    rev: u64,
    ws: Workspace,
    new_id: Box<dyn FnMut() -> Id + Send>,
}

impl Core {
    pub fn new(ws: Workspace) -> Self {
        Self::with_ids(ws, Box::new(crate::new_id))
    }

    /// A core with its own id source, so tests get stable ids.
    pub fn with_ids(ws: Workspace, new_id: Box<dyn FnMut() -> Id + Send>) -> Self {
        Self { rev: 1, ws, new_id }
    }

    /// One window with one side tab holding one tab: what a first start shows.
    pub fn first_start(&mut self) -> Id {
        self.first_start_with(1).0
    }

    /// Like `first_start`, with `tabs` tabs (at least one) in the side tab. Returns the window
    /// id and the pane ids in order, one per tab.
    pub fn first_start_with(&mut self, tabs: usize) -> (Id, Vec<Id>) {
        let window_id = (self.new_id)();
        let side_tab = self.new_side_tab_with(tabs);
        let panes = side_tab
            .tabs
            .iter()
            .flat_map(|t| t.layout.panes())
            .cloned()
            .collect();
        self.ws.windows.push(Window {
            id: window_id.clone(),
            side_tabs: vec![side_tab],
        });
        (window_id, panes)
    }

    /// Replaces the workspace with a restored one, before any window has seen a snapshot.
    pub fn restore(&mut self, ws: Workspace) {
        self.ws = ws;
    }

    /// Appends a side tab of `tabs` tabs to a window, outside the patch protocol: only for the
    /// start, before any window has seen a snapshot. Returns the pane ids, one per tab.
    pub fn append_side_tab(&mut self, window_id: &str, tabs: usize) -> Option<Vec<Id>> {
        let side_tab = self.new_side_tab_with(tabs);
        let panes = side_tab
            .tabs
            .iter()
            .flat_map(|t| t.layout.panes())
            .cloned()
            .collect();
        self.ws
            .windows
            .iter_mut()
            .find(|w| w.id == window_id)?
            .side_tabs
            .push(side_tab);
        Some(panes)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            rev: self.rev,
            workspace: self.ws.clone(),
        }
    }

    pub fn workspace(&self) -> &Workspace {
        &self.ws
    }

    /// A new tab whose icon also differs from `others`, and whose neon differs from theirs:
    /// its side tab's tabs, or the tabs made in the same command.
    fn new_tab_beside(&mut self, others: &[Tab]) -> Tab {
        let id = (self.new_id)();
        let also: Vec<Option<String>> = others.iter().map(|t| t.icon.clone()).collect();
        let icon = Some(pick_icon(&self.ws, &id, &also));
        let accent = Some(pick_tab_neon(others, &id));
        Tab {
            id,
            title: None,
            layout: Layout::pane((self.new_id)()),
            status: None,
            icon,
            accent,
        }
    }

    fn new_side_tab(&mut self) -> SideTab {
        self.new_side_tab_with(1)
    }

    fn new_side_tab_with(&mut self, tabs: usize) -> SideTab {
        let mut made: Vec<Tab> = Vec::new();
        for _ in 0..tabs.max(1) {
            let tab = self.new_tab_beside(&made);
            made.push(tab);
        }
        let tabs = made;
        let id = (self.new_id)();
        let accent = Some(pick_neon(&self.ws, &id));
        let name = Some(pick_name(&self.ws, &id));
        SideTab {
            name,
            id,
            title: None,
            tabs,
            status: None,
            icon: None,
            folder: false,
            accent,
        }
    }

    /// Checks a command and turns it into patches, without changing the workspace.
    fn plan(&mut self, cmd: Command) -> Result<Vec<Patch>, CommandError> {
        let ws = &self.ws;
        let source_empties = |tab_id: &str, target: Option<&str>| -> Option<Patch> {
            let (s, _) = ws.locate_tab(tab_id)?;
            (s.tabs.len() == 1 && !s.folder && Some(s.id.as_str()) != target).then(|| {
                Patch::SideTabClosed {
                    side_tab_id: s.id.clone(),
                }
            })
        };
        Ok(match cmd {
            Command::CreateWindow => {
                let window_id = (self.new_id)();
                let side_tab = self.new_side_tab();
                vec![
                    Patch::WindowCreated {
                        window_id: window_id.clone(),
                    },
                    Patch::SideTabCreated {
                        window_id,
                        index: 0,
                        side_tab,
                    },
                ]
            }
            Command::CreateSideTab { window_id, index } => {
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                let index = index.unwrap_or(w.side_tabs.len());
                if index > w.side_tabs.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let side_tab = self.new_side_tab();
                vec![Patch::SideTabCreated {
                    window_id,
                    index,
                    side_tab,
                }]
            }
            Command::CreateFolder { window_id, index } => {
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                let index = index.unwrap_or(w.side_tabs.len());
                if index > w.side_tabs.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let id = (self.new_id)();
                let side_tab = SideTab {
                    accent: Some(pick_neon(ws, &id)),
                    name: Some(pick_name(ws, &id)),
                    id,
                    title: None,
                    tabs: vec![],
                    status: None,
                    icon: None,
                    folder: true,
                };
                vec![Patch::SideTabCreated {
                    window_id,
                    index,
                    side_tab,
                }]
            }
            Command::CreateTab { side_tab_id } => {
                let siblings = ws
                    .side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?
                    .tabs
                    .clone();
                let index = siblings.len();
                let tab = self.new_tab_beside(&siblings);
                vec![Patch::TabCreated {
                    side_tab_id,
                    index,
                    tab,
                }]
            }
            Command::CloseTab { tab_id } => {
                ws.locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let closed = source_empties(&tab_id, None);
                std::iter::once(Patch::TabClosed { tab_id })
                    .chain(closed)
                    .collect()
            }
            Command::MoveTab {
                tab_id,
                to_side_tab_id,
                index,
            } => {
                let (from, _) = ws
                    .locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let target = ws
                    .side_tab(&to_side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(to_side_tab_id.clone()))?;
                let len = target.tabs.len() - usize::from(from.id == target.id);
                if index > len {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let closed = source_empties(&tab_id, Some(&to_side_tab_id));
                std::iter::once(Patch::TabMoved {
                    tab_id,
                    to_side_tab_id,
                    index,
                })
                .chain(closed)
                .collect()
            }
            Command::MoveTabToNewSideTab {
                tab_id,
                window_id,
                index,
            } => {
                let (from, _) = ws
                    .locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                if from.tabs.len() == 1 && !from.folder {
                    let side_tab_id = from.id.clone();
                    return self.plan(Command::MoveSideTab {
                        side_tab_id,
                        window_id,
                        index,
                    });
                }
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                if index > w.side_tabs.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let closed = source_empties(&tab_id, None);
                let id = (self.new_id)();
                let side_tab = SideTab {
                    accent: Some(pick_neon(ws, &id)),
                    name: Some(pick_name(ws, &id)),
                    id,
                    title: None,
                    tabs: vec![],
                    status: None,
                    icon: None,
                    folder: false,
                };
                let to_side_tab_id = side_tab.id.clone();
                [
                    Some(Patch::SideTabCreated {
                        window_id,
                        index,
                        side_tab,
                    }),
                    Some(Patch::TabMoved {
                        tab_id,
                        to_side_tab_id,
                        index: 0,
                    }),
                    closed,
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            Command::MoveTabToNewWindow { tab_id } => {
                let (from, _) = ws
                    .locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                if from.tabs.len() == 1 && !from.folder {
                    let side_tab_id = from.id.clone();
                    return self.plan(Command::MoveSideTabToNewWindow { side_tab_id });
                }
                let closed = source_empties(&tab_id, None);
                let window_id = (self.new_id)();
                let id = (self.new_id)();
                let side_tab = SideTab {
                    accent: Some(pick_neon(ws, &id)),
                    name: Some(pick_name(ws, &id)),
                    id,
                    title: None,
                    tabs: vec![],
                    status: None,
                    icon: None,
                    folder: false,
                };
                let to_side_tab_id = side_tab.id.clone();
                [
                    Some(Patch::WindowCreated {
                        window_id: window_id.clone(),
                    }),
                    Some(Patch::SideTabCreated {
                        window_id,
                        index: 0,
                        side_tab,
                    }),
                    Some(Patch::TabMoved {
                        tab_id,
                        to_side_tab_id,
                        index: 0,
                    }),
                    closed,
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            Command::SetStatus { target_id, status } => {
                if ws.tab(&target_id).is_none() && ws.side_tab(&target_id).is_none() {
                    return Err(CommandError::UnknownTab(target_id));
                }
                let status = status
                    .map(|mut s| {
                        s.look.label = s.look.label.trim().to_string();
                        s.look
                            .check()
                            .map(|_| s)
                            .map_err(|e| CommandError::BadStatus(e.to_string()))
                    })
                    .transpose()?;
                vec![Patch::StatusSet {
                    id: target_id,
                    status,
                }]
            }
            Command::RestyleStatus { status_id, status } => {
                if let Some(s) = &status {
                    s.look
                        .check()
                        .map_err(|e| CommandError::BadStatus(e.to_string()))?;
                }
                let uses = |m: &Option<ManualStatus>| {
                    m.as_ref()
                        .is_some_and(|m| m.status_id.as_deref() == Some(status_id.as_str()))
                };
                let status = status.map(|s| ManualStatus {
                    status_id: Some(status_id.clone()),
                    look: s.look,
                });
                let mut patches = Vec::new();
                for side in ws.windows.iter().flat_map(|w| w.side_tabs.iter()) {
                    if uses(&side.status) {
                        patches.push(Patch::StatusSet {
                            id: side.id.clone(),
                            status: status.clone(),
                        });
                    }
                    for tab in side.tabs.iter().filter(|t| uses(&t.status)) {
                        patches.push(Patch::StatusSet {
                            id: tab.id.clone(),
                            status: status.clone(),
                        });
                    }
                }
                patches
            }
            Command::MergeSideTab { from, into } => {
                if from == into {
                    return Err(CommandError::UnknownSideTab(from));
                }
                let source = ws
                    .side_tab(&from)
                    .ok_or(CommandError::UnknownSideTab(from.clone()))?;
                let target = ws
                    .side_tab(&into)
                    .ok_or(CommandError::UnknownSideTab(into.clone()))?;
                let len = target.tabs.len();
                // A one-tab row's name often sits on the row; in a folder each tab shows its own
                // name, so the name moves to the tab and nothing the person named is lost.
                let keep = |s: &SideTab| match (s.tabs.as_slice(), &s.title) {
                    ([t], Some(title)) if t.title.is_none() && !s.folder => {
                        Some(Patch::TabRenamed {
                            tab_id: t.id.clone(),
                            title: Some(title.clone()),
                        })
                    }
                    _ => None,
                };
                let mut patches: Vec<Patch> =
                    keep(target).into_iter().chain(keep(source)).collect();
                patches.extend(
                    source
                        .tabs
                        .iter()
                        .enumerate()
                        .map(|(i, t)| Patch::TabMoved {
                            tab_id: t.id.clone(),
                            to_side_tab_id: into.clone(),
                            index: len + i,
                        }),
                );
                patches.push(Patch::SideTabClosed { side_tab_id: from });
                patches
            }
            Command::RestoreSideTab {
                window_id,
                index,
                side_tab,
                tab_ids,
            } => {
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                let unique: std::collections::HashSet<&Id> = tab_ids.iter().collect();
                if ws.side_tab(&side_tab.id).is_some()
                    || tab_ids.is_empty()
                    || unique.len() != tab_ids.len()
                {
                    return Err(CommandError::UnknownSideTab(side_tab.id));
                }
                // A tab must not leave a side tab empty behind it.
                let mut left: std::collections::HashMap<&str, usize> = Default::default();
                for t in &tab_ids {
                    let (s, _) = ws
                        .locate_tab(t)
                        .ok_or(CommandError::UnknownTab(t.clone()))?;
                    let n = left.entry(s.id.as_str()).or_insert(s.tabs.len());
                    *n = n
                        .checked_sub(1)
                        .ok_or(CommandError::UnknownTab(t.clone()))?;
                }
                if let Some((s, _)) = left.iter().find(|(_, n)| **n == 0) {
                    return Err(CommandError::UnknownSideTab((*s).to_string()));
                }
                let side_tab_id = side_tab.id.clone();
                let mut patches = vec![Patch::SideTabCreated {
                    window_id,
                    index: index.min(w.side_tabs.len()),
                    side_tab: SideTab {
                        tabs: Vec::new(),
                        ..side_tab
                    },
                }];
                for (i, t) in tab_ids.into_iter().enumerate() {
                    patches.push(Patch::TabMoved {
                        tab_id: t,
                        to_side_tab_id: side_tab_id.clone(),
                        index: i,
                    });
                }
                patches
            }
            Command::ReopenSideTab {
                window_id,
                index,
                side_tab,
            } => {
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                if (side_tab.tabs.is_empty() && !side_tab.folder)
                    || ws.side_tab(&side_tab.id).is_some()
                    || !side_tab.tabs.iter().all(|t| absent(ws, t))
                {
                    return Err(CommandError::UnknownSideTab(side_tab.id));
                }
                vec![Patch::SideTabCreated {
                    window_id,
                    index: index.min(w.side_tabs.len()),
                    side_tab,
                }]
            }
            Command::ReopenTab {
                window_id,
                side_tab_id,
                index,
                tab,
            } => {
                if !absent(ws, &tab) {
                    return Err(CommandError::UnknownTab(tab.id));
                }
                if let Some(side) = ws.side_tab(&side_tab_id) {
                    return Ok(vec![Patch::TabCreated {
                        side_tab_id,
                        index: index.min(side.tabs.len()),
                        tab,
                    }]);
                }
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                let id = (self.new_id)();
                let side_tab = SideTab {
                    accent: Some(pick_neon(ws, &id)),
                    name: Some(pick_name(ws, &id)),
                    id,
                    title: None,
                    tabs: vec![tab],
                    status: None,
                    icon: None,
                    folder: false,
                };
                vec![Patch::SideTabCreated {
                    window_id,
                    index: w.side_tabs.len(),
                    side_tab,
                }]
            }
            Command::RenameTab { tab_id, title } => {
                ws.locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let title = title
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty());
                vec![Patch::TabRenamed { tab_id, title }]
            }
            Command::RenameSideTab { side_tab_id, title } => {
                ws.side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?;
                let title = title
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty());
                vec![Patch::SideTabRenamed { side_tab_id, title }]
            }
            Command::SetIcon { target_id, icon } => {
                if ws.tab(&target_id).is_none() && ws.side_tab(&target_id).is_none() {
                    return Err(CommandError::UnknownTab(target_id));
                }
                if let Some(i) = icon.as_deref().filter(|i| !ICONS.contains(i)) {
                    return Err(CommandError::BadIcon(i.to_string()));
                }
                vec![Patch::IconSet {
                    id: target_id,
                    icon,
                }]
            }
            Command::SetAccent { target_id, accent } => {
                if ws.tab(&target_id).is_none() && ws.side_tab(&target_id).is_none() {
                    return Err(CommandError::UnknownTab(target_id));
                }
                if !NEONS.contains(&accent.as_str()) {
                    return Err(CommandError::BadAccent(accent));
                }
                vec![Patch::AccentSet {
                    id: target_id,
                    accent,
                }]
            }
            Command::CloseSideTab { side_tab_id } => {
                ws.side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?;
                vec![Patch::SideTabClosed { side_tab_id }]
            }
            Command::MoveSideTabToNewWindow { side_tab_id } => {
                let side_tab = ws
                    .side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?
                    .clone();
                // The same side tab, id and all, so its saved context follows it.
                let window_id = (self.new_id)();
                vec![
                    Patch::WindowCreated {
                        window_id: window_id.clone(),
                    },
                    Patch::SideTabClosed { side_tab_id },
                    Patch::SideTabCreated {
                        window_id,
                        index: 0,
                        side_tab,
                    },
                ]
            }
            Command::MoveSideTab {
                side_tab_id,
                window_id,
                index,
            } => {
                let side_tab = ws
                    .side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?
                    .clone();
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                if index > w.side_tabs.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                // Counted after it left: a gap below it in its own window moves up one.
                let at = w.side_tabs.iter().position(|s| s.id == side_tab_id);
                let index = match at {
                    Some(i) if i < index => index - 1,
                    _ => index,
                };
                if at == Some(index) {
                    return Ok(vec![]);
                }
                // The same side tab, id and all, so its saved context follows it.
                vec![
                    Patch::SideTabClosed { side_tab_id },
                    Patch::SideTabCreated {
                        window_id,
                        index,
                        side_tab,
                    },
                ]
            }
            Command::MergeAllWindows { window_id } => {
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                let start = w.side_tabs.len();
                // The same side tabs, ids and all, so their saved contexts follow them.
                ws.windows
                    .iter()
                    .filter(|o| o.id != window_id)
                    .flat_map(|o| o.side_tabs.iter())
                    .enumerate()
                    .flat_map(|(i, side_tab)| {
                        [
                            Patch::SideTabClosed {
                                side_tab_id: side_tab.id.clone(),
                            },
                            Patch::SideTabCreated {
                                window_id: window_id.clone(),
                                index: start + i,
                                side_tab: side_tab.clone(),
                            },
                        ]
                    })
                    .collect()
            }
            Command::SplitPane { .. } | Command::OpenTranscript { .. } => {
                let (pane_id, dir, new) = match cmd {
                    Command::SplitPane { pane_id, dir } => {
                        (pane_id, dir, Layout::pane((self.new_id)()))
                    }
                    Command::OpenTranscript { pane_id, session } => {
                        if !crate::runtime::is_session_id(&session) {
                            return Err(CommandError::BadSession(session));
                        }
                        (
                            pane_id,
                            Dir::Row,
                            Layout::transcript((self.new_id)(), session),
                        )
                    }
                    _ => unreachable!(),
                };
                let (_, tab) = ws
                    .locate_pane(&pane_id)
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                let tab_id = tab.id.clone();
                let current = tab.layout.clone();
                let layout =
                    current
                        .split(&pane_id, dir, new, (self.new_id)())
                        .map_err(|e| match e {
                            LayoutError::TooSmall => CommandError::PaneTooSmall(pane_id),
                            _ => CommandError::UnknownPane(pane_id),
                        })?;
                vec![Patch::LayoutChanged { tab_id, layout }]
            }
            Command::DockTab {
                tab_id,
                pane_id,
                side,
            } => {
                let (from, index) = ws
                    .locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let moving = from.tabs[index].layout.clone();
                let (into_side, into) = ws
                    .locate_pane(&pane_id)
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                if into.id == tab_id {
                    return Err(CommandError::DockOntoItself(tab_id));
                }
                let (dir, before) = side.place();
                let layout = into
                    .layout
                    .split_at(&pane_id, dir, before, moving, (self.new_id)())
                    .map_err(|e| match e {
                        LayoutError::TooSmall => CommandError::PaneTooSmall(pane_id.clone()),
                        _ => CommandError::UnknownPane(pane_id.clone()),
                    })?;
                let closed = source_empties(&tab_id, Some(&into_side.id));
                [
                    Patch::LayoutChanged {
                        tab_id: into.id.clone(),
                        layout,
                    },
                    Patch::TabClosed { tab_id },
                ]
                .into_iter()
                .chain(closed)
                .collect()
            }
            Command::PaneToTab { pane_id, index } => {
                let (side, tab) = ws
                    .locate_pane(&pane_id)
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                if index > side.tabs.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let pane = tab
                    .layout
                    .leaf(&pane_id)
                    .cloned()
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                let left = tab
                    .layout
                    .remove(&pane_id)
                    .map_err(|_| CommandError::UnknownPane(pane_id.clone()))?
                    .ok_or(CommandError::OnlyPane(pane_id.clone()))?;
                let id = (self.new_id)();
                let new = Tab {
                    icon: Some(pick_icon(ws, &id, &[])),
                    accent: Some(pick_tab_neon(&side.tabs, &id)),
                    id,
                    title: None,
                    layout: pane,
                    status: None,
                };
                vec![
                    Patch::LayoutChanged {
                        tab_id: tab.id.clone(),
                        layout: left,
                    },
                    Patch::TabCreated {
                        side_tab_id: side.id.clone(),
                        index,
                        tab: new,
                    },
                ]
            }
            Command::OpenText {
                side_tab_id,
                doc_id,
                beside,
            } => {
                let side = ws
                    .side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?;
                let pane = Layout::text((self.new_id)(), doc_id);
                match beside {
                    Some(pane_id) => {
                        let tab = side
                            .tabs
                            .iter()
                            .find(|t| t.layout.has_pane(&pane_id))
                            .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                        let layout = tab
                            .layout
                            .split(&pane_id, Dir::Row, pane, (self.new_id)())
                            .map_err(|e| match e {
                                LayoutError::TooSmall => CommandError::PaneTooSmall(pane_id),
                                _ => CommandError::UnknownPane(pane_id),
                            })?;
                        vec![Patch::LayoutChanged {
                            tab_id: tab.id.clone(),
                            layout,
                        }]
                    }
                    None => vec![Patch::TabCreated {
                        side_tab_id,
                        index: side.tabs.len(),
                        tab: {
                            let id = (self.new_id)();
                            Tab {
                                icon: Some(pick_icon(ws, &id, &[])),
                                accent: Some(pick_tab_neon(&side.tabs, &id)),
                                id,
                                title: None,
                                layout: pane,
                                status: None,
                            }
                        },
                    }],
                }
            }
            Command::ClosePane { pane_id } => {
                let (_, tab) = ws
                    .locate_pane(&pane_id)
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                // A transcript pane has no shell: a tab left with only those closes too.
                let left = tab
                    .layout
                    .remove(&pane_id)
                    .map(|l| l.filter(Layout::holds_work));
                match left {
                    Ok(Some(layout)) => vec![Patch::LayoutChanged {
                        tab_id: tab.id.clone(),
                        layout,
                    }],
                    Ok(None) => {
                        let tab_id = tab.id.clone();
                        let closed = source_empties(&tab_id, None);
                        std::iter::once(Patch::TabClosed { tab_id })
                            .chain(closed)
                            .collect()
                    }
                    Err(_) => return Err(CommandError::UnknownPane(pane_id)),
                }
            }
            Command::ResizeSplit { split_id, sizes } => {
                let tab = ws
                    .windows
                    .iter()
                    .flat_map(|w| w.side_tabs.iter())
                    .flat_map(|s| s.tabs.iter())
                    .find(|t| t.layout.has_split(&split_id))
                    .ok_or(CommandError::UnknownSplit(split_id.clone()))?;
                let layout = tab.layout.resize(&split_id, &sizes).map_err(|e| match e {
                    LayoutError::BadSizes => CommandError::BadSizes(split_id.clone()),
                    _ => CommandError::UnknownSplit(split_id.clone()),
                })?;
                vec![Patch::LayoutChanged {
                    tab_id: tab.id.clone(),
                    layout,
                }]
            }
            Command::ResumePane { pane_id } | Command::DismissResume { pane_id } => {
                let (_, tab) = ws
                    .locate_pane(&pane_id)
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                if tab.layout.resume_of(&pane_id).is_none() {
                    return Err(CommandError::NothingToResume(pane_id));
                }
                let mut layout = tab.layout.clone();
                if let Some(Layout::Pane { resume, .. }) = layout.leaf_mut(&pane_id) {
                    *resume = None;
                }
                vec![Patch::LayoutChanged {
                    tab_id: tab.id.clone(),
                    layout,
                }]
            }
        })
    }

    pub fn handle(&mut self, cmd: Command) -> Result<PatchBatch, CommandError> {
        let mut patches = self.plan(cmd)?;
        let had: Vec<Id> = self
            .ws
            .windows
            .iter()
            .filter(|w| !w.side_tabs.is_empty())
            .map(|w| w.id.clone())
            .collect();
        for p in &patches {
            apply(&mut self.ws, p).expect("a planned patch applies");
        }
        // A window the batch emptied closes, unless it is the last one.
        let emptied: Vec<Id> = self
            .ws
            .windows
            .iter()
            .filter(|w| w.side_tabs.is_empty() && had.contains(&w.id))
            .map(|w| w.id.clone())
            .collect();
        for window_id in emptied {
            if self.ws.windows.len() < 2 {
                break;
            }
            let p = Patch::WindowClosed { window_id };
            apply(&mut self.ws, &p).expect("an empty window closes");
            patches.push(p);
        }
        self.rev += 1;
        Ok(PatchBatch {
            rev: self.rev,
            patches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Ids from `core()`: window id1, tab id2 with pane id3, side tab id4. Each new tab takes two
    // ids (tab, then pane), each new side tab one more after its tab.
    fn core() -> Core {
        let mut n = 0;
        let mut c = Core::with_ids(
            Workspace::default(),
            Box::new(move || {
                n += 1;
                format!("id{n}")
            }),
        );
        c.first_start();
        c
    }

    fn shape(c: &Core) -> Vec<Vec<Vec<String>>> {
        c.workspace()
            .windows
            .iter()
            .map(|w| {
                w.side_tabs
                    .iter()
                    .map(|s| s.tabs.iter().map(|t| t.id.clone()).collect())
                    .collect()
            })
            .collect()
    }

    fn create_tab(c: &mut Core, side: &str) {
        c.handle(Command::CreateTab {
            side_tab_id: side.into(),
        })
        .unwrap();
    }

    #[test]
    fn a_tab_dropped_outside_opens_a_window_and_the_emptied_one_closes() {
        let mut c = core();
        create_tab(&mut c, "id4"); // id5, pane id6
        let b = c
            .handle(Command::MoveTabToNewWindow {
                tab_id: "id5".into(),
            })
            .unwrap();
        // New window id7 with side tab id8.
        assert_eq!(shape(&c), vec![vec![vec!["id2"]], vec![vec!["id5"]]]);
        assert_eq!(c.workspace().windows[1].id, "id7");
        assert!(matches!(&b.patches[0], Patch::WindowCreated { window_id } if window_id == "id7"));
        assert!(
            !b.patches
                .iter()
                .any(|p| matches!(p, Patch::WindowClosed { .. }))
        );

        // Its only tab back into the first window: the second window closes.
        let b = c
            .handle(Command::MoveTab {
                tab_id: "id5".into(),
                to_side_tab_id: "id4".into(),
                index: 1,
            })
            .unwrap();
        assert_eq!(shape(&c), vec![vec![vec!["id2", "id5"]]]);
        assert_eq!(
            b.patches.last(),
            Some(&Patch::WindowClosed {
                window_id: "id7".into()
            })
        );
        // Every patch in the batch replays on a copy, as the windows apply them.
        let mut copy = Workspace::default();
        copy.windows.push(Window {
            id: "id1".into(),
            side_tabs: vec![],
        });
        assert!(
            apply(
                &mut copy,
                &Patch::WindowClosed {
                    window_id: "id9".into()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn the_last_window_stays_open_when_it_empties() {
        let mut c = core();
        c.handle(Command::MoveTabToNewWindow {
            tab_id: "id2".into(),
        })
        .unwrap();
        // The first window lost its only tab and closed; one window remains.
        assert_eq!(shape(&c), vec![vec![vec!["id2"]]]);
        c.handle(Command::CloseTab {
            tab_id: "id2".into(),
        })
        .unwrap();
        assert_eq!(c.workspace().windows.len(), 1);
    }

    #[test]
    fn first_start_has_one_side_tab_with_one_tab_of_one_pane() {
        let c = core();
        assert_eq!(shape(&c), vec![vec![vec!["id2"]]]);
        let w = &c.workspace().windows[0];
        assert_eq!(w.id, "id1");
        assert_eq!(w.side_tabs[0].id, "id4");
        assert_eq!(w.side_tabs[0].tabs[0].layout, Layout::pane("id3"));
    }

    #[test]
    fn create_tab_appends_and_bumps_rev() {
        let mut c = core();
        let b = c
            .handle(Command::CreateTab {
                side_tab_id: "id4".into(),
            })
            .unwrap();
        assert_eq!(b.rev, 2);
        assert_eq!(shape(&c), vec![vec![vec!["id2", "id5"]]]);
    }

    #[test]
    fn merge_windows_moves_every_row() {
        let mut c = core();
        let first = c.workspace().windows[0].id.clone();
        let rows = |c: &Core| -> Vec<Vec<Id>> {
            c.workspace()
                .windows
                .iter()
                .map(|w| w.side_tabs.iter().map(|s| s.id.clone()).collect())
                .collect()
        };
        c.handle(Command::CreateWindow).unwrap();
        c.handle(Command::CreateWindow).unwrap();
        let before = rows(&c);
        let b = c
            .handle(Command::MergeAllWindows {
                window_id: first.clone(),
            })
            .unwrap();
        let ws = c.workspace();
        assert_eq!(ws.windows.len(), 1, "the emptied windows close");
        assert_eq!(
            rows(&c),
            vec![before.concat()],
            "rows keep their ids, in window order"
        );
        let closed = b
            .patches
            .iter()
            .filter(|p| matches!(p, Patch::WindowClosed { .. }))
            .count();
        assert_eq!(closed, 2);
        // With one window it changes nothing.
        assert!(
            c.handle(Command::MergeAllWindows { window_id: first })
                .unwrap()
                .patches
                .is_empty()
        );
    }

    #[test]
    fn create_window_opens_a_window_with_one_new_tab() {
        let mut c = core();
        let before = c.workspace().windows[0].clone();
        c.handle(Command::CreateWindow).unwrap();
        let ws = c.workspace();
        assert_eq!(ws.windows.len(), 2);
        assert_eq!(ws.windows[0], before, "the first window keeps its rows");
        let w = &ws.windows[1];
        assert_eq!(w.side_tabs.len(), 1);
        assert_eq!(w.side_tabs[0].tabs.len(), 1);
    }

    #[test]
    fn a_folder_starts_empty_and_outlives_its_last_tab() {
        let mut c = core();
        c.handle(Command::CreateFolder {
            window_id: "id1".into(),
            index: Some(0),
        })
        .unwrap();
        let folder = c.workspace().windows[0].side_tabs[0].clone();
        assert!(folder.folder && folder.tabs.is_empty() && folder.name.is_some());
        let other = c.workspace().windows[0].side_tabs[1].clone();
        let tab = other.tabs[0].id.clone();
        // A row's only tab moves in; the row it left closes, the folder keeps it.
        c.handle(Command::MoveTab {
            tab_id: tab.clone(),
            to_side_tab_id: folder.id.clone(),
            index: 0,
        })
        .unwrap();
        let w = &c.workspace().windows[0];
        assert_eq!(w.side_tabs.len(), 1);
        assert_eq!(w.side_tabs[0].tabs[0].id, tab);
        // Out to its own row, the tab goes alone and the folder stays.
        c.handle(Command::MoveTabToNewSideTab {
            tab_id: tab.clone(),
            window_id: "id1".into(),
            index: 1,
        })
        .unwrap();
        let w = &c.workspace().windows[0];
        assert_eq!(w.side_tabs.len(), 2);
        assert!(w.side_tabs[0].tabs.is_empty());
        // Merged into, the folder takes no row name onto the tab.
        c.handle(Command::RenameSideTab {
            side_tab_id: folder.id.clone(),
            title: Some("infra".into()),
        })
        .unwrap();
        let row = c.workspace().windows[0].side_tabs[1].id.clone();
        c.handle(Command::MergeSideTab {
            from: row,
            into: folder.id.clone(),
        })
        .unwrap();
        let f = &c.workspace().windows[0].side_tabs[0];
        assert_eq!((f.title.as_deref(), f.tabs.len()), (Some("infra"), 1));
        assert_eq!(f.tabs[0].title, None);
        // Its last tab closes; the folder stays.
        c.handle(Command::CloseTab { tab_id: tab }).unwrap();
        assert_eq!(c.workspace().windows[0].side_tabs.len(), 1);
        assert!(c.workspace().windows[0].side_tabs[0].tabs.is_empty());
    }

    #[test]
    fn a_merge_keeps_the_names_of_one_tab_rows() {
        let mut c = core();
        c.handle(Command::CreateSideTab {
            window_id: "id1".into(),
            index: None,
        })
        .unwrap();
        let second = c.workspace().windows[0].side_tabs[1].clone();
        let tab = second.tabs[0].id.clone();
        for (id, title) in [("id4", "web"), (second.id.as_str(), "api")] {
            c.handle(Command::RenameSideTab {
                side_tab_id: id.into(),
                title: Some(title.into()),
            })
            .unwrap();
        }
        c.handle(Command::MergeSideTab {
            from: second.id.clone(),
            into: "id4".into(),
        })
        .unwrap();
        let folder = &c.workspace().windows[0].side_tabs[0];
        let names: Vec<Option<&str>> = folder.tabs.iter().map(|t| t.title.as_deref()).collect();
        assert_eq!(names, [Some("web"), Some("api")]);
        assert_eq!(folder.tabs[1].id, tab);
        // A tab the person named keeps that name over its row's.
        c.handle(Command::CreateSideTab {
            window_id: "id1".into(),
            index: None,
        })
        .unwrap();
        let third = c.workspace().windows[0].side_tabs[1].clone();
        c.handle(Command::RenameSideTab {
            side_tab_id: third.id.clone(),
            title: Some("row".into()),
        })
        .unwrap();
        c.handle(Command::RenameTab {
            tab_id: third.tabs[0].id.clone(),
            title: Some("logs".into()),
        })
        .unwrap();
        c.handle(Command::MergeSideTab {
            from: third.id,
            into: "id4".into(),
        })
        .unwrap();
        let folder = &c.workspace().windows[0].side_tabs[0];
        assert_eq!(folder.tabs[2].title.as_deref(), Some("logs"));
    }

    #[test]
    fn merge_moves_every_tab_and_restore_puts_them_back() {
        let mut c = core();
        // A second side tab id6 (tab id5, pane …) after id4, with two tabs.
        c.handle(Command::CreateSideTab {
            window_id: "id1".into(),
            index: None,
        })
        .unwrap();
        let second = c.workspace().windows[0].side_tabs[1].clone();
        create_tab(&mut c, &second.id);
        let moved: Vec<Id> = c.workspace().windows[0].side_tabs[1]
            .tabs
            .iter()
            .map(|t| t.id.clone())
            .collect();
        c.handle(Command::MergeSideTab {
            from: second.id.clone(),
            into: "id4".into(),
        })
        .unwrap();
        let w = &c.workspace().windows[0];
        assert_eq!(w.side_tabs.len(), 1);
        let ids: Vec<&str> = w.side_tabs[0].tabs.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["id2", moved[0].as_str(), moved[1].as_str()]);
        assert!(
            c.handle(Command::MergeSideTab {
                from: "id4".into(),
                into: "id4".into(),
            })
            .is_err()
        );

        c.handle(Command::RestoreSideTab {
            window_id: "id1".into(),
            index: 1,
            side_tab: second.clone(),
            tab_ids: moved.clone(),
        })
        .unwrap();
        let w = &c.workspace().windows[0];
        assert_eq!(w.side_tabs[1].id, second.id);
        let back: Vec<Id> = w.side_tabs[1].tabs.iter().map(|t| t.id.clone()).collect();
        assert_eq!(back, moved);
        // Restoring would empty id4 now: refused.
        let err = c.handle(Command::RestoreSideTab {
            window_id: "id1".into(),
            index: 0,
            side_tab: SideTab {
                folder: false,
                accent: None,
                name: None,
                id: "fresh".into(),
                title: None,
                tabs: Vec::new(),
                status: None,
                icon: None,
            },
            tab_ids: vec!["id2".into()],
        });
        assert!(err.is_err());
        let dup = c.handle(Command::RestoreSideTab {
            window_id: "id1".into(),
            index: 0,
            side_tab: SideTab {
                folder: false,
                accent: None,
                name: None,
                id: "fresh".into(),
                title: None,
                tabs: Vec::new(),
                status: None,
                icon: None,
            },
            tab_ids: vec![moved[0].clone(), moved[0].clone()],
        });
        assert!(dup.is_err());
    }

    #[test]
    fn reorder_inside_a_side_tab() {
        let mut c = core();
        create_tab(&mut c, "id4");
        create_tab(&mut c, "id4");
        c.handle(Command::MoveTab {
            tab_id: "id2".into(),
            to_side_tab_id: "id4".into(),
            index: 2,
        })
        .unwrap();
        assert_eq!(shape(&c), vec![vec![vec!["id5", "id7", "id2"]]]);
        let err = c.handle(Command::MoveTab {
            tab_id: "id2".into(),
            to_side_tab_id: "id4".into(),
            index: 3,
        });
        assert_eq!(err, Err(CommandError::IndexOutOfRange(3)));
    }

    #[test]
    fn moving_the_last_tab_closes_its_side_tab() {
        let mut c = core();
        c.handle(Command::CreateSideTab {
            window_id: "id1".into(),
            index: None,
        })
        .unwrap();
        // The new side tab id7 holds tab id5.
        let b = c
            .handle(Command::MoveTab {
                tab_id: "id5".into(),
                to_side_tab_id: "id4".into(),
                index: 0,
            })
            .unwrap();
        assert_eq!(
            b.patches.last(),
            Some(&Patch::SideTabClosed {
                side_tab_id: "id7".into()
            })
        );
        assert_eq!(shape(&c), vec![vec![vec!["id5", "id2"]]]);
    }

    #[test]
    fn closing_the_last_tab_closes_its_side_tab() {
        let mut c = core();
        let b = c
            .handle(Command::CloseTab {
                tab_id: "id2".into(),
            })
            .unwrap();
        assert_eq!(b.patches.len(), 2);
        assert_eq!(shape(&c), vec![Vec::<Vec<String>>::new()]);
    }

    #[test]
    fn move_to_a_sidebar_gap_makes_a_new_side_tab() {
        let mut c = core();
        create_tab(&mut c, "id4");
        c.handle(Command::MoveTabToNewSideTab {
            tab_id: "id5".into(),
            window_id: "id1".into(),
            index: 0,
        })
        .unwrap();
        assert_eq!(shape(&c), vec![vec![vec!["id5"], vec!["id2"]]]);
        // Its only tab moved out again: the new side tab closes, the old one stays.
        c.handle(Command::MoveTab {
            tab_id: "id5".into(),
            to_side_tab_id: "id4".into(),
            index: 1,
        })
        .unwrap();
        assert_eq!(shape(&c), vec![vec![vec!["id2", "id5"]]]);
    }

    fn side_ids(c: &Core, w: usize) -> Vec<String> {
        c.workspace().windows[w]
            .side_tabs
            .iter()
            .map(|s| s.id.clone())
            .collect()
    }

    #[test]
    fn a_side_tab_moves_to_a_gap_and_keeps_its_id() {
        let mut c = core();
        c.handle(Command::CreateSideTab {
            window_id: "id1".into(),
            index: None,
        })
        .unwrap();
        let before = c.workspace().side_tab("id7").unwrap().clone();
        let to = |c: &mut Core, index| {
            c.handle(Command::MoveSideTab {
                side_tab_id: "id7".into(),
                window_id: "id1".into(),
                index,
            })
            .unwrap()
        };
        to(&mut c, 0);
        assert_eq!(side_ids(&c, 0), ["id7", "id4"]);
        assert_eq!(c.workspace().side_tab("id7"), Some(&before));
        // The gaps on either side of it leave it where it is.
        assert!(to(&mut c, 1).patches.is_empty());
        to(&mut c, 2);
        assert_eq!(side_ids(&c, 0), ["id4", "id7"]);
        assert!(
            c.handle(Command::MoveSideTab {
                side_tab_id: "id7".into(),
                window_id: "id1".into(),
                index: 3,
            })
            .is_err()
        );
    }

    #[test]
    fn a_window_s_only_side_tab_moves_to_another_window_and_its_window_closes() {
        let mut c = core();
        c.handle(Command::CreateSideTab {
            window_id: "id1".into(),
            index: None,
        })
        .unwrap();
        c.handle(Command::MoveSideTabToNewWindow {
            side_tab_id: "id4".into(),
        })
        .unwrap();
        let away = c.workspace().windows[1].id.clone();
        let moving = c.workspace().side_tab("id4").unwrap().clone();
        let home = c.workspace().windows[0].id.clone();
        let b = c
            .handle(Command::MoveSideTab {
                side_tab_id: "id4".into(),
                window_id: home.clone(),
                index: 1,
            })
            .unwrap();
        assert_ne!(away, home);
        assert!(b.patches.contains(&Patch::WindowClosed { window_id: away }));
        assert_eq!(c.workspace().windows.len(), 1);
        assert_eq!(c.workspace().windows[0].side_tabs[1], moving);
    }

    #[test]
    fn the_only_tab_of_a_side_tab_moves_its_side_tab() {
        let mut c = core();
        c.handle(Command::CreateSideTab {
            window_id: "id1".into(),
            index: None,
        })
        .unwrap();
        // Tab id5 is side tab id7's only one: a gap moves id7, so its context stays.
        c.handle(Command::MoveTabToNewSideTab {
            tab_id: "id5".into(),
            window_id: "id1".into(),
            index: 0,
        })
        .unwrap();
        assert_eq!(side_ids(&c, 0), ["id7", "id4"]);
        // Outside every window, id7 opens a window of its own.
        c.handle(Command::MoveTabToNewWindow {
            tab_id: "id5".into(),
        })
        .unwrap();
        assert_eq!(side_ids(&c, 1), ["id7"]);
        // Back into the first window: the second, emptied, closes.
        let b = c
            .handle(Command::MoveSideTab {
                side_tab_id: "id7".into(),
                window_id: "id1".into(),
                index: 1,
            })
            .unwrap();
        assert_eq!(c.workspace().windows.len(), 1);
        assert_eq!(side_ids(&c, 0), ["id4", "id7"]);
        assert!(
            b.patches
                .iter()
                .any(|p| matches!(p, Patch::WindowClosed { .. }))
        );
    }

    #[test]
    fn a_side_tab_renames_closes_and_moves_to_a_new_window() {
        let mut c = core();
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        c.handle(Command::RenameSideTab {
            side_tab_id: side.clone(),
            title: Some(" PROJ-482 ".into()),
        })
        .unwrap();
        assert_eq!(
            c.workspace().side_tab(&side).unwrap().title.as_deref(),
            Some("PROJ-482")
        );
        c.handle(Command::RenameSideTab {
            side_tab_id: side.clone(),
            title: Some("  ".into()),
        })
        .unwrap();
        assert_eq!(c.workspace().side_tab(&side).unwrap().title, None);

        let window = c.workspace().windows[0].id.clone();
        c.handle(Command::CreateSideTab {
            window_id: window,
            index: None,
        })
        .unwrap();
        let before = c.workspace().side_tab(&side).unwrap().clone();
        let b = c
            .handle(Command::MoveSideTabToNewWindow {
                side_tab_id: side.clone(),
            })
            .unwrap();
        // The same side tab, with its tabs, alone in a new window; the old window stays.
        assert_eq!(c.workspace().windows.len(), 2);
        assert_eq!(c.workspace().windows[1].side_tabs, vec![before]);
        assert!(matches!(&b.patches[0], Patch::WindowCreated { .. }));

        // Closing it empties the new window, which closes; the first stays.
        let b = c
            .handle(Command::CloseSideTab {
                side_tab_id: side.clone(),
            })
            .unwrap();
        assert!(c.workspace().side_tab(&side).is_none());
        assert_eq!(c.workspace().windows.len(), 1);
        assert!(
            b.patches
                .iter()
                .any(|p| matches!(p, Patch::WindowClosed { .. }))
        );
        assert!(
            c.handle(Command::CloseSideTab { side_tab_id: side })
                .is_err()
        );
    }

    #[test]
    fn new_tabs_open_with_different_icons() {
        let mut c = core();
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        for _ in 0..4 {
            c.handle(Command::CreateTab {
                side_tab_id: side.clone(),
            })
            .unwrap();
        }
        let icons: Vec<String> = c.workspace().side_tab(&side).unwrap().tabs[1..]
            .iter()
            .map(|t| t.icon.clone().expect("a new tab has an icon"))
            .collect();
        let mut unique = icons.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), icons.len(), "{icons:?}");
        assert!(
            icons
                .iter()
                .all(|i| ICONS.contains(&i.as_str()) && i != "agent")
        );
    }

    #[test]
    fn new_side_tabs_open_with_different_neons() {
        let mut c = core();
        let window = c.workspace().windows[0].id.clone();
        for _ in 0..5 {
            c.handle(Command::CreateSideTab {
                window_id: window.clone(),
                index: None,
            })
            .unwrap();
        }
        let neons: Vec<String> = c.workspace().windows[0]
            .side_tabs
            .iter()
            .map(|s| s.accent.clone().expect("a new side tab has a neon"))
            .collect();
        let mut unique = neons.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), neons.len(), "{neons:?}");
        assert!(neons.iter().all(|n| NEONS.contains(&n.as_str())));

        // A tab moved out to a row of its own gets a neon too.
        let tab = c.workspace().windows[0].side_tabs[0].tabs[0].id.clone();
        c.handle(Command::MoveTabToNewSideTab {
            tab_id: tab,
            window_id: window,
            index: 0,
        })
        .unwrap();
        assert!(c.workspace().windows[0].side_tabs[0].accent.is_some());
    }

    #[test]
    fn tabs_of_a_side_tab_open_with_different_neons() {
        let mut c = core();
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        for _ in 0..4 {
            c.handle(Command::CreateTab {
                side_tab_id: side.clone(),
            })
            .unwrap();
        }
        let neons: Vec<String> = c
            .workspace()
            .side_tab(&side)
            .unwrap()
            .tabs
            .iter()
            .map(|t| t.accent.clone().expect("a new tab has a neon"))
            .collect();
        let mut unique = neons.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 5, "{neons:?}");
    }

    #[test]
    fn side_tab_names_stay_unique_and_number_once_the_list_runs_out() {
        let mut c = core();
        let window = c.workspace().windows[0].id.clone();
        for _ in 0..NAMES.len() + 2 {
            c.handle(Command::CreateSideTab {
                window_id: window.clone(),
                index: None,
            })
            .unwrap();
        }
        let names: Vec<String> = c.workspace().windows[0]
            .side_tabs
            .iter()
            .map(|s| s.name.clone().unwrap())
            .collect();
        let unique: HashSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "no two side tabs share a name");
        assert!(
            names[..NAMES.len()]
                .iter()
                .all(|n| NAMES.contains(&n.as_str()))
        );
        // Past the list, a number follows a name: "Otter 2".
        let extra = &names[NAMES.len()..];
        assert!(extra.iter().all(|n| n.ends_with(" 2") || n.ends_with(" 3")));

        let mut ws = c.workspace().clone();
        ws.windows[0].side_tabs[0].name = None;
        let taken = ws.windows[0].side_tabs[1].name.clone();
        fill_accents(&mut ws);
        let filled = ws.windows[0].side_tabs[0].name.clone();
        assert!(filled.is_some() && filled != taken);
    }

    #[test]
    fn rows_and_tabs_from_before_neons_get_one() {
        let mut c = core();
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        c.handle(Command::CreateTab {
            side_tab_id: side.clone(),
        })
        .unwrap();
        let mut ws = c.workspace().clone();
        let s = &mut ws.windows[0].side_tabs[0];
        s.accent = None;
        let kept = s.tabs[1].accent.clone();
        s.tabs[0].accent = None;
        s.tabs[0].icon = None;
        fill_accents(&mut ws);
        let s = &ws.windows[0].side_tabs[0];
        assert!(s.tabs[0].icon.is_some() && s.tabs[0].icon != s.tabs[1].icon);
        assert!(s.accent.is_some());
        assert_eq!(s.tabs[1].accent, kept);
        assert!(s.tabs[0].accent.is_some() && s.tabs[0].accent != kept);
    }

    #[test]
    fn a_colour_comes_from_the_neons() {
        let mut c = core();
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        for (id, neon) in [(side.as_str(), NEONS[0]), ("id2", NEONS[1])] {
            c.handle(Command::SetAccent {
                target_id: id.into(),
                accent: neon.into(),
            })
            .unwrap();
        }
        let s = c.workspace().side_tab(&side).unwrap();
        assert_eq!(s.accent.as_deref(), Some(NEONS[0]));
        assert_eq!(s.tabs[0].accent.as_deref(), Some(NEONS[1]));
        assert_eq!(
            c.handle(Command::SetAccent {
                target_id: side.clone(),
                accent: "amber".into(),
            }),
            Err(CommandError::BadAccent("amber".into()))
        );
        assert_eq!(
            c.handle(Command::SetAccent {
                target_id: "nope".into(),
                accent: NEONS[0].into(),
            }),
            Err(CommandError::UnknownTab("nope".into()))
        );
    }

    #[test]
    fn an_icon_comes_from_the_list_and_resets() {
        let mut c = core();
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        c.handle(Command::SetIcon {
            target_id: side.clone(),
            icon: Some("pin".into()),
        })
        .unwrap();
        c.handle(Command::SetIcon {
            target_id: "id2".into(),
            icon: Some("doc".into()),
        })
        .unwrap();
        let s = c.workspace().side_tab(&side).unwrap();
        assert_eq!(s.icon.as_deref(), Some("pin"));
        assert_eq!(s.tabs[0].icon.as_deref(), Some("doc"));
        assert_eq!(
            c.handle(Command::SetIcon {
                target_id: side.clone(),
                icon: Some("rocket".into()),
            }),
            Err(CommandError::BadIcon("rocket".into()))
        );
        assert_eq!(
            c.handle(Command::SetIcon {
                target_id: "nope".into(),
                icon: None,
            }),
            Err(CommandError::UnknownTab("nope".into()))
        );
        c.handle(Command::SetIcon {
            target_id: side.clone(),
            icon: None,
        })
        .unwrap();
        assert_eq!(c.workspace().side_tab(&side).unwrap().icon, None);
    }

    #[test]
    fn rename_trims_and_clears() {
        let mut c = core();
        c.handle(Command::RenameTab {
            tab_id: "id2".into(),
            title: Some("  api  ".into()),
        })
        .unwrap();
        assert_eq!(
            c.workspace().windows[0].side_tabs[0].tabs[0]
                .title
                .as_deref(),
            Some("api")
        );
        c.handle(Command::RenameTab {
            tab_id: "id2".into(),
            title: Some("   ".into()),
        })
        .unwrap();
        assert_eq!(c.workspace().windows[0].side_tabs[0].tabs[0].title, None);
    }

    #[test]
    fn a_text_pane_opens_as_a_tab_or_beside_a_pane_and_runs_no_shell() {
        let mut c = core();
        // A new tab at the end of the side tab.
        c.handle(Command::OpenText {
            side_tab_id: "id4".into(),
            doc_id: "doc-1".into(),
            beside: None,
        })
        .unwrap();
        let side = &c.workspace().windows[0].side_tabs[0];
        assert_eq!(side.tabs.len(), 2);
        let text_tab = side.tabs[1].clone();
        assert_eq!(text_tab.layout.terminals(), Vec::<&Id>::new());
        assert_eq!(text_tab.layout.docs().len(), 1);
        let json = serde_json::to_string(&text_tab.layout).unwrap();
        assert!(json.contains(r#""doc":"doc-1""#), "{json}");
        // Beside the shell pane: a split to the right.
        c.handle(Command::OpenText {
            side_tab_id: "id4".into(),
            doc_id: "doc-2".into(),
            beside: Some("id3".into()),
        })
        .unwrap();
        let first = c.workspace().tab("id2").unwrap().layout.clone();
        assert_eq!(first.terminals(), vec!["id3"]);
        let (text_pane, _) = first.docs()[0];
        let text_pane = text_pane.clone();
        assert_eq!(c.workspace().doc_panes().len(), 2);
        // Closing the shell leaves the text pane, so the tab stays.
        c.handle(Command::ClosePane {
            pane_id: "id3".into(),
        })
        .unwrap();
        assert!(c.workspace().tab("id2").is_some());
        c.handle(Command::ClosePane { pane_id: text_pane }).unwrap();
        assert!(c.workspace().tab("id2").is_none());
        assert_eq!(
            c.handle(Command::OpenText {
                side_tab_id: "nope".into(),
                doc_id: "doc-3".into(),
                beside: None,
            }),
            Err(CommandError::UnknownSideTab("nope".into()))
        );
    }

    #[test]
    fn split_adds_a_pane_and_closing_panes_ends_with_the_tab() {
        let mut c = core();
        let b = c
            .handle(Command::SplitPane {
                pane_id: "id3".into(),
                dir: Dir::Row,
            })
            .unwrap();
        // New pane id5, split id6.
        let layout = Layout::pane("id3")
            .split("id3", Dir::Row, Layout::pane("id5"), "id6".into())
            .unwrap();
        assert_eq!(
            b.patches,
            vec![Patch::LayoutChanged {
                tab_id: "id2".into(),
                layout: layout.clone()
            }]
        );
        assert_eq!(c.workspace().locate_pane("id5").unwrap().1.id, "id2");
        c.handle(Command::ResizeSplit {
            split_id: "id6".into(),
            sizes: vec![1, 3],
        })
        .unwrap();
        assert_eq!(
            c.workspace().tab("id2").unwrap().layout,
            layout.resize("id6", &[250, 750]).unwrap()
        );
        assert_eq!(
            c.handle(Command::ResizeSplit {
                split_id: "id6".into(),
                sizes: vec![1, 999],
            }),
            Err(CommandError::BadSizes("id6".into()))
        );
        c.handle(Command::ClosePane {
            pane_id: "id3".into(),
        })
        .unwrap();
        assert_eq!(
            c.workspace().tab("id2").unwrap().layout,
            Layout::pane("id5")
        );
        let b = c
            .handle(Command::ClosePane {
                pane_id: "id5".into(),
            })
            .unwrap();
        assert_eq!(b.patches.len(), 2, "the tab and its side tab close");
        assert_eq!(shape(&c), vec![Vec::<Vec<String>>::new()]);
        assert_eq!(
            c.handle(Command::ClosePane {
                pane_id: "id5".into()
            }),
            Err(CommandError::UnknownPane("id5".into()))
        );
    }

    #[test]
    fn a_transcript_pane_opens_beside_and_runs_no_shell() {
        let mut c = core();
        assert_eq!(
            c.handle(Command::OpenTranscript {
                pane_id: "id3".into(),
                session: "--resume".into()
            }),
            Err(CommandError::BadSession("--resume".into()))
        );
        c.handle(Command::OpenTranscript {
            pane_id: "id3".into(),
            session: "sess-0b5c".into(),
        })
        .unwrap();
        let layout = &c.workspace().windows[0].side_tabs[0].tabs[0].layout;
        assert_eq!(layout.panes().len(), 2);
        assert_eq!(layout.terminals(), vec!["id3"]);
        assert_eq!(c.workspace().terminal_sides().len(), 1);
        let json = serde_json::to_string(layout).unwrap();
        assert!(json.contains(r#""transcript":"sess-0b5c""#), "{json}");
        c.handle(Command::ClosePane {
            pane_id: "id3".into(),
        })
        .unwrap();
        assert_eq!(
            shape(&c),
            vec![Vec::<Vec<String>>::new()],
            "a tab left with only a transcript closes"
        );
    }

    #[test]
    fn resume_clears_the_offer_and_needs_one() {
        let mut c = core();
        assert_eq!(
            c.handle(Command::ResumePane {
                pane_id: "id3".into()
            }),
            Err(CommandError::NothingToResume("id3".into()))
        );
        let mut ws = c.workspace().clone();
        ws.windows[0].side_tabs[0].tabs[0].layout = Layout::Pane {
            id: "id3".into(),
            resume: Some("sess-1".into()),
            transcript: None,
            doc: None,
        };
        c.restore(ws);
        c.handle(Command::DismissResume {
            pane_id: "id3".into(),
        })
        .unwrap();
        assert_eq!(
            c.workspace().tab("id2").unwrap().layout,
            Layout::pane("id3")
        );
    }

    #[test]
    fn a_rejected_command_changes_nothing() {
        let mut c = core();
        let before = c.snapshot();
        assert!(
            c.handle(Command::CloseTab {
                tab_id: "nope".into()
            })
            .is_err()
        );
        assert!(
            c.handle(Command::SplitPane {
                pane_id: "nope".into(),
                dir: Dir::Column
            })
            .is_err()
        );
        assert_eq!(c.snapshot(), before);
    }

    #[test]
    fn patches_replayed_on_a_copy_give_the_same_workspace() {
        let mut c = core();
        let mut copy = c.snapshot().workspace;
        let cmds = [
            // Tab id5 (pane id6) in side tab id4.
            Command::CreateTab {
                side_tab_id: "id4".into(),
            },
            // Side tab id9 with tab id7 (pane id8).
            Command::CreateSideTab {
                window_id: "id1".into(),
                index: Some(0),
            },
            Command::MoveTab {
                tab_id: "id2".into(),
                to_side_tab_id: "id9".into(),
                index: 1,
            },
            Command::MoveTabToNewSideTab {
                tab_id: "id7".into(),
                window_id: "id1".into(),
                index: 2,
            },
            // Pane id11 beside id3, split id12; then pane id13 below id11, split id14.
            Command::SplitPane {
                pane_id: "id3".into(),
                dir: Dir::Row,
            },
            Command::SplitPane {
                pane_id: "id11".into(),
                dir: Dir::Column,
            },
            Command::ClosePane {
                pane_id: "id3".into(),
            },
            Command::CloseTab {
                tab_id: "id5".into(),
            },
        ];
        for cmd in cmds {
            for p in c.handle(cmd).unwrap().patches {
                apply(&mut copy, &p).unwrap();
            }
        }
        assert_eq!(&copy, c.workspace());
        let panes: Vec<String> = c
            .workspace()
            .pane_sides()
            .into_iter()
            .map(|p| p.0)
            .collect();
        assert_eq!(panes, ["id11", "id13", "id8"]);
    }

    #[test]
    fn json_shape_matches_the_typescript_host() {
        let cmd = Command::MoveTab {
            tab_id: "t".into(),
            to_side_tab_id: "s".into(),
            index: 1,
        };
        assert_eq!(
            serde_json::to_string(&cmd).unwrap(),
            r#"{"type":"MoveTab","tabId":"t","toSideTabId":"s","index":1}"#
        );
        let cmd = Command::SplitPane {
            pane_id: "p".into(),
            dir: Dir::Column,
        };
        assert_eq!(
            serde_json::to_string(&cmd).unwrap(),
            r#"{"type":"SplitPane","paneId":"p","dir":"column"}"#
        );
        let err = CommandError::UnknownTab("t".into());
        assert_eq!(
            serde_json::to_string(&err).unwrap(),
            r#"{"code":"UnknownTab","id":"t"}"#
        );
    }
}

#[cfg(test)]
mod dock_tests {
    use super::*;

    fn core() -> Core {
        let mut n = 0;
        let mut c = Core::with_ids(
            Workspace::default(),
            Box::new(move || {
                n += 1;
                format!("id{n}")
            }),
        );
        c.first_start();
        c
    }

    fn first_side(c: &Core) -> &SideTab {
        &c.workspace().windows[0].side_tabs[0]
    }

    /// A side tab with two one-pane tabs: (side, first tab, its pane, second tab, its pane).
    fn two_tabs() -> (Core, Id, Id, Id, Id, Id) {
        let mut c = core();
        let side = first_side(&c).id.clone();
        c.handle(Command::CreateTab {
            side_tab_id: side.clone(),
        })
        .unwrap();
        let s = first_side(&c);
        let pane = |t: &Tab| t.layout.panes()[0].clone();
        let (a, b) = (s.tabs[0].clone(), s.tabs[1].clone());
        (c, side, a.id.clone(), pane(&a), b.id.clone(), pane(&b))
    }

    fn order(l: &Layout) -> (Option<Dir>, Vec<Id>) {
        let dir = match l {
            Layout::Split { dir, .. } => Some(*dir),
            Layout::Pane { .. } => None,
        };
        (dir, l.panes().into_iter().cloned().collect())
    }

    #[test]
    fn a_tab_docks_on_each_side_of_a_pane_and_closes() {
        for (side, dir, first) in [
            (DockSide::Left, Dir::Row, true),
            (DockSide::Right, Dir::Row, false),
            (DockSide::Up, Dir::Column, true),
            (DockSide::Down, Dir::Column, false),
        ] {
            let (mut c, _, a, pa, b, pb) = two_tabs();
            let batch = c
                .handle(Command::DockTab {
                    tab_id: b.clone(),
                    pane_id: pa.clone(),
                    side,
                })
                .unwrap();
            assert!(
                batch
                    .patches
                    .contains(&Patch::TabClosed { tab_id: b.clone() })
            );
            let s = first_side(&c);
            assert_eq!(s.tabs.len(), 1, "{side:?}");
            let want = if first {
                vec![pb.clone(), pa.clone()]
            } else {
                vec![pa.clone(), pb.clone()]
            };
            assert_eq!(
                order(&c.workspace().tab(&a).unwrap().layout),
                (Some(dir), want)
            );
        }
    }

    #[test]
    fn a_split_tab_docks_whole_and_merges_into_a_split_of_its_direction() {
        let (mut c, _, a, pa, b, pb) = two_tabs();
        c.handle(Command::SplitPane {
            pane_id: pb.clone(),
            dir: Dir::Row,
        })
        .unwrap();
        let pb2 = c.workspace().tab(&b).unwrap().layout.panes()[1].clone();
        c.handle(Command::DockTab {
            tab_id: b,
            pane_id: pa.clone(),
            side: DockSide::Right,
        })
        .unwrap();
        // Three columns, not a column holding a split.
        let layout = &c.workspace().tab(&a).unwrap().layout;
        assert_eq!(order(layout), (Some(Dir::Row), vec![pa, pb, pb2]));
        let Layout::Split { sizes, .. } = layout else {
            panic!("a split")
        };
        assert_eq!(
            sizes.iter().map(|&s| u32::from(s)).sum::<u32>(),
            u32::from(crate::layout::TOTAL)
        );
        assert_eq!(sizes[0], crate::layout::TOTAL - crate::layout::TOTAL / 2);
    }

    #[test]
    fn docking_refuses_its_own_pane_and_a_pane_too_small() {
        let (mut c, _, a, pa, _, _) = two_tabs();
        assert_eq!(
            c.handle(Command::DockTab {
                tab_id: a.clone(),
                pane_id: pa.clone(),
                side: DockSide::Left,
            }),
            Err(CommandError::DockOntoItself(a.clone()))
        );
        // Split the first pane until it cannot halve again, then dock beside it.
        let mut c2 = c;
        let mut target = pa.clone();
        loop {
            match c2.handle(Command::SplitPane {
                pane_id: target.clone(),
                dir: Dir::Row,
            }) {
                Ok(_) => {
                    let tab = c2.workspace().tab(&a).unwrap();
                    target = tab.layout.panes().last().unwrap().to_string();
                }
                Err(CommandError::PaneTooSmall(_)) => break,
                Err(e) => panic!("{e}"),
            }
        }
        let other = first_side(&c2).tabs[1].id.clone();
        assert_eq!(
            c2.handle(Command::DockTab {
                tab_id: other,
                pane_id: target.clone(),
                side: DockSide::Right,
            }),
            Err(CommandError::PaneTooSmall(target))
        );
    }

    #[test]
    fn the_only_tab_of_a_side_tab_docks_and_its_side_tab_closes() {
        let mut c = core();
        let w = c.workspace().windows[0].id.clone();
        let pa = first_side(&c).tabs[0].layout.panes()[0].clone();
        c.handle(Command::CreateSideTab {
            window_id: w,
            index: None,
        })
        .unwrap();
        let other = c.workspace().windows[0].side_tabs[1].clone();
        let b = c
            .handle(Command::DockTab {
                tab_id: other.tabs[0].id.clone(),
                pane_id: pa,
                side: DockSide::Down,
            })
            .unwrap();
        assert!(b.patches.contains(&Patch::SideTabClosed {
            side_tab_id: other.id
        }));
        assert_eq!(c.workspace().windows[0].side_tabs.len(), 1);
        assert_eq!(first_side(&c).tabs[0].layout.terminals().len(), 2);
    }

    #[test]
    fn a_pane_leaves_its_tab_for_a_new_tab_and_the_only_pane_stays() {
        let (mut c, side, a, pa, b, _) = two_tabs();
        assert_eq!(
            c.handle(Command::PaneToTab {
                pane_id: pa.clone(),
                index: 0
            }),
            Err(CommandError::OnlyPane(pa.clone()))
        );
        c.handle(Command::SplitPane {
            pane_id: pa.clone(),
            dir: Dir::Column,
        })
        .unwrap();
        let pa2 = c.workspace().tab(&a).unwrap().layout.panes()[1].clone();
        c.handle(Command::PaneToTab {
            pane_id: pa2.clone(),
            index: 1,
        })
        .unwrap();
        let s = c.workspace().side_tab(&side).unwrap();
        assert_eq!(s.tabs.len(), 3);
        assert_eq!(s.tabs[0].id, a);
        assert_eq!(s.tabs[2].id, b);
        let new = &s.tabs[1];
        assert_eq!(new.layout, Layout::pane(pa2));
        assert!(new.icon.is_some() && new.accent.is_some());
        assert_eq!(c.workspace().tab(&a).unwrap().layout, Layout::pane(pa));
        assert_eq!(
            c.handle(Command::PaneToTab {
                pane_id: "nope".into(),
                index: 0
            }),
            Err(CommandError::UnknownPane("nope".into()))
        );
    }

    #[test]
    fn dock_commands_read_as_the_ui_sends_them() {
        let cmd: Command =
            serde_json::from_str(r#"{"type":"DockTab","tabId":"t","paneId":"p","side":"up"}"#)
                .unwrap();
        assert_eq!(
            cmd,
            Command::DockTab {
                tab_id: "t".into(),
                pane_id: "p".into(),
                side: DockSide::Up
            }
        );
        let cmd: Command =
            serde_json::from_str(r#"{"type":"PaneToTab","paneId":"p","index":2}"#).unwrap();
        assert_eq!(
            cmd,
            Command::PaneToTab {
                pane_id: "p".into(),
                index: 2
            }
        );
    }
}
