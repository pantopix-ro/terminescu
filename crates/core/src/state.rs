//! The workspace model and its command and patch protocol (D6).
//!
//! A window sends a [`Command`]; the core checks it against the workspace, turns it into a
//! [`PatchBatch`], applies the batch to its own copy and broadcasts it to every window. Each
//! window applies the same patches to its store, so all copies stay equal. `rev` numbers the
//! batches; a window that sees a gap asks for a [`Snapshot`].

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Command {
    /// A new side tab with one new tab, at `index` in the window's sidebar (end when absent).
    CreateSideTab { window_id: Id, index: Option<usize> },
    /// A new tab at the end of a side tab.
    CreateTab { side_tab_id: Id },
    CloseTab { tab_id: Id },
    /// Reorder inside a side tab or move into another one. `index` is the position in the
    /// target after the tab left its old place.
    MoveTab { tab_id: Id, to_side_tab_id: Id, index: usize },
    /// Drop on a sidebar gap: a new side tab at `index` holding the tab (D5).
    MoveTabToNewSideTab { tab_id: Id, window_id: Id, index: usize },
    RenameTab { tab_id: Id, title: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Patch {
    SideTabCreated { window_id: Id, index: usize, side_tab: SideTab },
    SideTabClosed { side_tab_id: Id },
    TabCreated { side_tab_id: Id, index: usize, tab: Tab },
    TabClosed { tab_id: Id },
    TabMoved { tab_id: Id, to_side_tab_id: Id, index: usize },
    TabRenamed { tab_id: Id, title: Option<String> },
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
}

impl Workspace {
    fn side_tab_mut(&mut self, id: &str) -> Option<&mut SideTab> {
        self.windows.iter_mut().flat_map(|w| w.side_tabs.iter_mut()).find(|s| s.id == id)
    }

    pub fn side_tab(&self, id: &str) -> Option<&SideTab> {
        self.windows.iter().flat_map(|w| w.side_tabs.iter()).find(|s| s.id == id)
    }

    /// The side tab holding a tab, and the tab's index in it.
    pub fn locate_tab(&self, tab_id: &str) -> Option<(&SideTab, usize)> {
        self.windows
            .iter()
            .flat_map(|w| w.side_tabs.iter())
            .find_map(|s| s.tabs.iter().position(|t| t.id == tab_id).map(|i| (s, i)))
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
        Patch::SideTabCreated { window_id, index, side_tab } => {
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
        Patch::TabCreated { side_tab_id, index, tab } => {
            let s = ws
                .side_tab_mut(side_tab_id)
                .ok_or_else(|| CommandError::UnknownSideTab(side_tab_id.clone()))?;
            if *index > s.tabs.len() {
                return Err(CommandError::IndexOutOfRange(*index));
            }
            s.tabs.insert(*index, tab.clone());
        }
        Patch::TabClosed { tab_id } => {
            ws.take_tab(tab_id).ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
        }
        Patch::TabMoved { tab_id, to_side_tab_id, index } => {
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
            let tab = ws.take_tab(tab_id).ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
            ws.side_tab_mut(to_side_tab_id).expect("checked above").tabs.insert(*index, tab);
        }
        Patch::TabRenamed { tab_id, title } => {
            let tab = ws
                .windows
                .iter_mut()
                .flat_map(|w| w.side_tabs.iter_mut())
                .flat_map(|s| s.tabs.iter_mut())
                .find(|t| &t.id == tab_id)
                .ok_or_else(|| CommandError::UnknownTab(tab_id.clone()))?;
            tab.title = title.clone();
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
        let window_id = (self.new_id)();
        let side_tab = self.new_side_tab();
        self.ws.windows.push(Window { id: window_id.clone(), side_tabs: vec![side_tab] });
        window_id
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { rev: self.rev, workspace: self.ws.clone() }
    }

    pub fn workspace(&self) -> &Workspace {
        &self.ws
    }

    fn new_side_tab(&mut self) -> SideTab {
        let tab = Tab { id: (self.new_id)(), title: None };
        SideTab { id: (self.new_id)(), title: None, tabs: vec![tab] }
    }

    /// Checks a command and turns it into patches, without changing the workspace.
    fn plan(&mut self, cmd: Command) -> Result<Vec<Patch>, CommandError> {
        let ws = &self.ws;
        let source_empties = |tab_id: &str, target: Option<&str>| -> Option<Patch> {
            let (s, _) = ws.locate_tab(tab_id)?;
            (s.tabs.len() == 1 && Some(s.id.as_str()) != target)
                .then(|| Patch::SideTabClosed { side_tab_id: s.id.clone() })
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
                vec![Patch::SideTabCreated { window_id, index, side_tab }]
            }
            Command::CreateTab { side_tab_id } => {
                let index = ws
                    .side_tab(&side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(side_tab_id.clone()))?
                    .tabs
                    .len();
                let tab = Tab { id: (self.new_id)(), title: None };
                vec![Patch::TabCreated { side_tab_id, index, tab }]
            }
            Command::CloseTab { tab_id } => {
                ws.locate_tab(&tab_id).ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let closed = source_empties(&tab_id, None);
                std::iter::once(Patch::TabClosed { tab_id }).chain(closed).collect()
            }
            Command::MoveTab { tab_id, to_side_tab_id, index } => {
                let (from, _) = ws.locate_tab(&tab_id).ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let target = ws
                    .side_tab(&to_side_tab_id)
                    .ok_or(CommandError::UnknownSideTab(to_side_tab_id.clone()))?;
                let len = target.tabs.len() - usize::from(from.id == target.id);
                if index > len {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let closed = source_empties(&tab_id, Some(&to_side_tab_id));
                std::iter::once(Patch::TabMoved { tab_id, to_side_tab_id, index }).chain(closed).collect()
            }
            Command::MoveTabToNewSideTab { tab_id, window_id, index } => {
                ws.locate_tab(&tab_id).ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let w = ws
                    .windows
                    .iter()
                    .find(|w| w.id == window_id)
                    .ok_or(CommandError::UnknownWindow(window_id.clone()))?;
                if index > w.side_tabs.len() {
                    return Err(CommandError::IndexOutOfRange(index));
                }
                let closed = source_empties(&tab_id, None);
                let side_tab = SideTab { id: (self.new_id)(), title: None, tabs: vec![] };
                let to_side_tab_id = side_tab.id.clone();
                [
                    Some(Patch::SideTabCreated { window_id, index, side_tab }),
                    Some(Patch::TabMoved { tab_id, to_side_tab_id, index: 0 }),
                    closed,
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            Command::RenameTab { tab_id, title } => {
                ws.locate_tab(&tab_id).ok_or(CommandError::UnknownTab(tab_id.clone()))?;
                let title = title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
                vec![Patch::TabRenamed { tab_id, title }]
            }
        })
    }

    pub fn handle(&mut self, cmd: Command) -> Result<PatchBatch, CommandError> {
        let patches = self.plan(cmd)?;
        for p in &patches {
            apply(&mut self.ws, p).expect("a planned patch applies");
        }
        self.rev += 1;
        Ok(PatchBatch { rev: self.rev, patches })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> Core {
        let mut n = 0;
        let mut c = Core::with_ids(Workspace::default(), Box::new(move || { n += 1; format!("id{n}") }));
        c.first_start();
        c
    }

    fn shape(c: &Core) -> Vec<Vec<Vec<String>>> {
        c.workspace()
            .windows
            .iter()
            .map(|w| w.side_tabs.iter().map(|s| s.tabs.iter().map(|t| t.id.clone()).collect()).collect())
            .collect()
    }

    #[test]
    fn first_start_has_one_side_tab_with_one_tab() {
        let c = core();
        assert_eq!(shape(&c), vec![vec![vec!["id2"]]]);
        assert_eq!(c.workspace().windows[0].id, "id1");
        assert_eq!(c.workspace().windows[0].side_tabs[0].id, "id3");
    }

    #[test]
    fn create_tab_appends_and_bumps_rev() {
        let mut c = core();
        let b = c.handle(Command::CreateTab { side_tab_id: "id3".into() }).unwrap();
        assert_eq!(b.rev, 2);
        assert_eq!(shape(&c), vec![vec![vec!["id2", "id4"]]]);
    }

    #[test]
    fn reorder_inside_a_side_tab() {
        let mut c = core();
        c.handle(Command::CreateTab { side_tab_id: "id3".into() }).unwrap();
        c.handle(Command::CreateTab { side_tab_id: "id3".into() }).unwrap();
        c.handle(Command::MoveTab { tab_id: "id2".into(), to_side_tab_id: "id3".into(), index: 2 }).unwrap();
        assert_eq!(shape(&c), vec![vec![vec!["id4", "id5", "id2"]]]);
        let err = c.handle(Command::MoveTab { tab_id: "id2".into(), to_side_tab_id: "id3".into(), index: 3 });
        assert_eq!(err, Err(CommandError::IndexOutOfRange(3)));
    }

    #[test]
    fn moving_the_last_tab_closes_its_side_tab() {
        let mut c = core();
        c.handle(Command::CreateSideTab { window_id: "id1".into(), index: None }).unwrap();
        // The new side tab id5 holds tab id4.
        let b = c.handle(Command::MoveTab { tab_id: "id4".into(), to_side_tab_id: "id3".into(), index: 0 }).unwrap();
        assert_eq!(b.patches.last(), Some(&Patch::SideTabClosed { side_tab_id: "id5".into() }));
        assert_eq!(shape(&c), vec![vec![vec!["id4", "id2"]]]);
    }

    #[test]
    fn closing_the_last_tab_closes_its_side_tab() {
        let mut c = core();
        let b = c.handle(Command::CloseTab { tab_id: "id2".into() }).unwrap();
        assert_eq!(b.patches.len(), 2);
        assert_eq!(shape(&c), vec![Vec::<Vec<String>>::new()]);
    }

    #[test]
    fn move_to_a_sidebar_gap_makes_a_new_side_tab() {
        let mut c = core();
        c.handle(Command::CreateTab { side_tab_id: "id3".into() }).unwrap();
        c.handle(Command::MoveTabToNewSideTab { tab_id: "id4".into(), window_id: "id1".into(), index: 0 }).unwrap();
        assert_eq!(shape(&c), vec![vec![vec!["id4"], vec!["id2"]]]);
        // Its only tab moved out again: the new side tab closes, the old one stays.
        c.handle(Command::MoveTab { tab_id: "id4".into(), to_side_tab_id: "id3".into(), index: 1 }).unwrap();
        assert_eq!(shape(&c), vec![vec![vec!["id2", "id4"]]]);
    }

    #[test]
    fn rename_trims_and_clears() {
        let mut c = core();
        c.handle(Command::RenameTab { tab_id: "id2".into(), title: Some("  api  ".into()) }).unwrap();
        assert_eq!(c.workspace().windows[0].side_tabs[0].tabs[0].title.as_deref(), Some("api"));
        c.handle(Command::RenameTab { tab_id: "id2".into(), title: Some("   ".into()) }).unwrap();
        assert_eq!(c.workspace().windows[0].side_tabs[0].tabs[0].title, None);
    }

    #[test]
    fn a_rejected_command_changes_nothing() {
        let mut c = core();
        let before = c.snapshot();
        assert!(c.handle(Command::CloseTab { tab_id: "nope".into() }).is_err());
        assert_eq!(c.snapshot(), before);
    }

    #[test]
    fn patches_replayed_on_a_copy_give_the_same_workspace() {
        let mut c = core();
        let mut copy = c.snapshot().workspace;
        let cmds = [
            Command::CreateTab { side_tab_id: "id3".into() },
            Command::CreateSideTab { window_id: "id1".into(), index: Some(0) },
            Command::MoveTab { tab_id: "id2".into(), to_side_tab_id: "id6".into(), index: 1 },
            Command::MoveTabToNewSideTab { tab_id: "id5".into(), window_id: "id1".into(), index: 2 },
            Command::CloseTab { tab_id: "id4".into() },
        ];
        for cmd in cmds {
            for p in c.handle(cmd).unwrap().patches {
                apply(&mut copy, &p).unwrap();
            }
        }
        assert_eq!(&copy, c.workspace());
    }

    #[test]
    fn json_shape_matches_the_typescript_host() {
        let cmd = Command::MoveTab { tab_id: "t".into(), to_side_tab_id: "s".into(), index: 1 };
        assert_eq!(
            serde_json::to_string(&cmd).unwrap(),
            r#"{"type":"MoveTab","tabId":"t","toSideTabId":"s","index":1}"#
        );
        let err = CommandError::UnknownTab("t".into());
        assert_eq!(serde_json::to_string(&err).unwrap(), r#"{"code":"UnknownTab","id":"t"}"#);
    }
}
