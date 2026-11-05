//! The workspace model and its command and patch protocol (D6).
//!
//! A window sends a [`Command`]; the core checks it against the workspace, turns it into a
//! [`PatchBatch`], applies the batch to its own copy and broadcasts it to every window. Each
//! window applies the same patches to its store, so all copies stay equal. `rev` numbers the
//! batches; a window that sees a gap asks for a [`Snapshot`].

use crate::layout::{Dir, Layout, LayoutError};
use serde::{Deserialize, Serialize};

pub type Id = String;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: Id,
    pub title: Option<String>,
    /// The tab's split tree; its leaves are panes (D3).
    pub layout: Layout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Command {
    /// A new side tab with one new tab, at `index` in the window's sidebar (end when absent).
    CreateSideTab {
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
    /// Drop on a sidebar gap: a new side tab at `index` holding the tab (D5).
    MoveTabToNewSideTab {
        tab_id: Id,
        window_id: Id,
        index: usize,
    },
    RenameTab {
        tab_id: Id,
        title: Option<String>,
    },
    /// A new pane beside `pane_id`: right for `row`, below for `column`.
    SplitPane {
        pane_id: Id,
        dir: Dir,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Patch {
    SideTabCreated {
        window_id: Id,
        index: usize,
        side_tab: SideTab,
    },
    SideTabClosed {
        side_tab_id: Id,
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
    /// The tab's whole split tree after a split, close, resize or resume change.
    LayoutChanged {
        tab_id: Id,
        layout: Layout,
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
        Patch::TabRenamed { tab_id, title } => {
            let tab = ws
                .tab_mut(tab_id)
                .ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
            tab.title = title.clone();
        }
        Patch::LayoutChanged { tab_id, layout } => {
            let tab = ws
                .tab_mut(tab_id)
                .ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
            tab.layout = layout.clone();
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

    /// A tab with one new pane.
    fn new_tab(&mut self) -> Tab {
        let id = (self.new_id)();
        Tab {
            id,
            title: None,
            layout: Layout::pane((self.new_id)()),
        }
    }

    fn new_side_tab(&mut self) -> SideTab {
        self.new_side_tab_with(1)
    }

    fn new_side_tab_with(&mut self, tabs: usize) -> SideTab {
        let tabs = (0..tabs.max(1)).map(|_| self.new_tab()).collect();
        SideTab {
            id: (self.new_id)(),
            title: None,
            tabs,
        }
    }

    /// Checks a command and turns it into patches, without changing the workspace.
    fn plan(&mut self, cmd: Command) -> Result<Vec<Patch>, CommandError> {
        let ws = &self.ws;
        let source_empties = |tab_id: &str, target: Option<&str>| -> Option<Patch> {
            let (s, _) = ws.locate_tab(tab_id)?;
            (s.tabs.len() == 1 && Some(s.id.as_str()) != target).then(|| Patch::SideTabClosed {
                side_tab_id: s.id.clone(),
            })
        };
        Ok(match cmd {
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
            Command::CreateTab { side_tab_id } => {
                let index = ws
                    .side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?
                    .tabs
                    .len();
                let tab = self.new_tab();
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
                ws.locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                if index > w.side_tabs.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let closed = source_empties(&tab_id, None);
                let side_tab = SideTab {
                    id: (self.new_id)(),
                    title: None,
                    tabs: vec![],
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
            Command::RenameTab { tab_id, title } => {
                ws.locate_tab(&tab_id)
                    .ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let title = title
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty());
                vec![Patch::TabRenamed { tab_id, title }]
            }
            Command::SplitPane { pane_id, dir } => {
                let (_, tab) = ws
                    .locate_pane(&pane_id)
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                let tab_id = tab.id.clone();
                let current = tab.layout.clone();
                let new = Layout::pane((self.new_id)());
                let layout =
                    current
                        .split(&pane_id, dir, new, (self.new_id)())
                        .map_err(|e| match e {
                            LayoutError::TooSmall => CommandError::PaneTooSmall(pane_id),
                            _ => CommandError::UnknownPane(pane_id),
                        })?;
                vec![Patch::LayoutChanged { tab_id, layout }]
            }
            Command::ClosePane { pane_id } => {
                let (_, tab) = ws
                    .locate_pane(&pane_id)
                    .ok_or(CommandError::UnknownPane(pane_id.clone()))?;
                match tab.layout.remove(&pane_id) {
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
        let patches = self.plan(cmd)?;
        for p in &patches {
            apply(&mut self.ws, p).expect("a planned patch applies");
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
