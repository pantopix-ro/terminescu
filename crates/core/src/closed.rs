//! Undo close (task 8.87). The last 10 closed tabs and rows, for this session only. A closed
//! row keeps a copy of its context rows, taken before the database deletes them with the row;
//! its session and secret variables stay in the daemon until the entry leaves the list.

use crate::db::{Db, Result};
use crate::layout::Layout;
use crate::state::{Command, Id, SideTab, Tab, Workspace};
use rusqlite::params_from_iter;
use rusqlite::types::Value;
use std::collections::{HashMap, VecDeque};

/// How many closed items the list keeps.
pub const KEEP: usize = 10;

/// The tables that hold a row's context, each with the rows that belong to it. Parents come
/// before children, so a restore inserts statuses before their rules.
const TABLES: [(&str, &str); 7] = [
    ("context_items", "side_tab_id = ?2"),
    ("pins", "side_tab_id = ?2"),
    ("context_files", "side_tab_id = ?2"),
    ("commands", "side_tab_id = ?2"),
    ("statuses", "side_tab_id = ?2"),
    (
        "status_rules",
        "status_id IN (SELECT id FROM statuses WHERE side_tab_id = ?2)",
    ),
    ("plugin_side_tab_data", "side_tab_id = ?2"),
];

#[derive(Debug, Clone)]
struct TableRows {
    table: &'static str,
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

/// A copy of a row's context: variables, pins, touched files, local commands, statuses and
/// plugin row data.
#[derive(Debug, Clone, Default)]
pub struct ContextRows(Vec<TableRows>);

impl ContextRows {
    pub fn take(db: &Db, side_tab_id: &str) -> Result<Self> {
        let owner = db.local_owner()?;
        let mut out = Vec::new();
        for (table, rows_of) in TABLES {
            let mut stmt = db.conn().prepare(&format!(
                "SELECT * FROM {table} WHERE owner_id = ?1 AND {rows_of}"
            ))?;
            let columns: Vec<String> = stmt.column_names().map(String::from).collect();
            let n = columns.len();
            let rows = stmt
                .query_map([&owner, side_tab_id], |r| {
                    (0..n).map(|i| r.get::<_, Value>(i)).collect()
                })?
                .collect::<rusqlite::Result<Vec<Vec<Value>>>>()?;
            out.push(TableRows {
                table,
                columns,
                rows,
            });
        }
        Ok(Self(out))
    }

    /// Puts the rows back once the row is in the database again. A row whose id came back
    /// some other way is kept as it is.
    pub fn restore(&self, db: &Db) -> Result<()> {
        let tx = db.conn().unchecked_transaction()?;
        for t in &self.0 {
            let marks = vec!["?"; t.columns.len()].join(", ");
            let sql = format!(
                "INSERT OR IGNORE INTO {} ({}) VALUES ({marks})",
                t.table,
                t.columns.join(", ")
            );
            let mut stmt = tx.prepare(&sql)?;
            for row in &t.rows {
                stmt.execute(params_from_iter(row.iter()))?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// What a close took away.
#[derive(Debug, Clone)]
pub enum Closed {
    /// A row, its context with it.
    Row {
        window_id: Id,
        index: usize,
        side_tab: SideTab,
        context: ContextRows,
    },
    /// One tab of a row that stays.
    Tab {
        window_id: Id,
        side_tab_id: Id,
        index: usize,
        tab: Tab,
    },
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub closed: Closed,
    /// Each pane's last cwd, where its fresh login shell starts.
    pub cwds: HashMap<Id, String>,
}

impl Entry {
    /// The id of the tab or row the entry brings back.
    pub fn key(&self) -> &Id {
        match &self.closed {
            Closed::Row { side_tab, .. } => &side_tab.id,
            Closed::Tab { tab, .. } => &tab.id,
        }
    }

    /// The row whose daemon variables wait on this entry.
    pub fn parked_side_tab(&self) -> Option<&Id> {
        match &self.closed {
            Closed::Row { side_tab, .. } => Some(&side_tab.id),
            Closed::Tab { .. } => None,
        }
    }

    /// The command that reopens the entry. Its window when the workspace lists it and it is
    /// `live` (a window closed with the red button has no view), else `fallback`.
    pub fn reopen(&self, ws: &Workspace, live: impl Fn(&Id) -> bool, fallback: &str) -> Command {
        let window = |id: &Id| {
            if live(id) && ws.windows.iter().any(|w| &w.id == id) {
                id.clone()
            } else {
                fallback.to_string()
            }
        };
        match &self.closed {
            Closed::Row {
                window_id,
                index,
                side_tab,
                ..
            } => Command::ReopenSideTab {
                window_id: window(window_id),
                index: *index,
                side_tab: side_tab.clone(),
            },
            Closed::Tab {
                window_id,
                side_tab_id,
                index,
                tab,
            } => Command::ReopenTab {
                window_id: window(window_id),
                side_tab_id: side_tab_id.clone(),
                index: *index,
                tab: tab.clone(),
            },
        }
    }
}

/// The closed list, newest last, and the closes still running. A running close's entry waits
/// apart, so a reopen never takes a tab or row that is still open.
#[derive(Debug, Default)]
pub struct ClosedList {
    done: VecDeque<Entry>,
    /// Each running close's entry, by the token [`ClosedList::push`] gave it.
    running: Vec<(u64, Entry)>,
    next: u64,
    /// The entry whose reopen failed last; a second failure in a row drops it.
    failed: Option<Id>,
}

impl ClosedList {
    pub const fn new() -> Self {
        Self {
            done: VecDeque::new(),
            running: Vec::new(),
            next: 0,
            failed: None,
        }
    }

    /// Adds an entry before its close runs, so the close leaves a row's daemon variables
    /// alone. Returns the token [`ClosedList::settle`] files it by when the close is over.
    pub fn push(&mut self, entry: Entry) -> u64 {
        self.next += 1;
        self.running.push((self.next, entry));
        self.next
    }

    /// The close of `token` is over. A done one joins the list; a failed one leaves quietly, as
    /// its tab or row is still open. Returns the oldest entries beyond [`KEEP`], which leave.
    pub fn settle(&mut self, token: u64, ok: bool) -> Vec<Entry> {
        if let Some(at) = self.running.iter().position(|(t, _)| *t == token) {
            let (_, entry) = self.running.remove(at);
            if ok {
                self.done.push_back(entry);
            }
        }
        let over = self.done.len().saturating_sub(KEEP);
        self.done.drain(..over).collect()
    }

    /// The newest closed tab or row whose close is over.
    pub fn pop(&mut self) -> Option<Entry> {
        self.done.pop_back()
    }

    /// An entry whose reopen failed goes back on top, so ⇧⌘T can try it again. One that fails a
    /// second time in a row comes back out to be dropped, so it cannot block the older ones.
    pub fn put_back(&mut self, entry: Entry) -> Option<Entry> {
        if self.failed.as_ref() == Some(entry.key()) {
            self.failed = None;
            return Some(entry);
        }
        self.failed = Some(entry.key().clone());
        self.done.push_back(entry);
        None
    }

    /// A reopen worked: an earlier failure no longer counts toward "in a row".
    pub fn reopened(&mut self) {
        self.failed = None;
    }

    pub fn is_parked(&self, side_tab_id: &str) -> bool {
        self.done
            .iter()
            .chain(self.running.iter().map(|(_, e)| e))
            .any(|e| e.parked_side_tab().map(String::as_str) == Some(side_tab_id))
    }

    pub fn drain(&mut self) -> Vec<Entry> {
        self.running
            .drain(..)
            .map(|(_, e)| e)
            .chain(self.done.drain(..))
            .collect()
    }
}

/// The command may close a tab or a row: the cheap test before [`closing`].
pub fn may_close(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::CloseSideTab { .. } | Command::CloseTab { .. } | Command::ClosePane { .. }
    )
}

/// The tab or row `cmd` would close, as it is now. None when it closes nothing, or nothing
/// with a shell: text and transcript panes do not come back.
pub fn closing(db: &Db, ws: &Workspace, cmd: &Command) -> Result<Option<Closed>> {
    let tab_id = match cmd {
        Command::CloseSideTab { side_tab_id } => return row(db, ws, side_tab_id),
        Command::CloseTab { tab_id } => tab_id.clone(),
        Command::ClosePane { pane_id } => {
            let Some((_, tab)) = ws.locate_pane(pane_id) else {
                return Ok(None);
            };
            let left = tab.layout.remove(pane_id).ok().flatten();
            if left.is_some_and(|l| l.holds_work()) {
                return Ok(None);
            }
            tab.id.clone()
        }
        _ => return Ok(None),
    };
    let Some((side, _)) = ws.locate_tab(&tab_id) else {
        return Ok(None);
    };
    if side.tabs.len() == 1 {
        return row(db, ws, &side.id);
    }
    let Some((window_id, _)) = place(ws, &side.id) else {
        return Ok(None);
    };
    let index = side.tabs.iter().position(|t| t.id == tab_id).unwrap_or(0);
    Ok(fresh_tab(&side.tabs[index]).map(|tab| Closed::Tab {
        window_id,
        side_tab_id: side.id.clone(),
        index,
        tab,
    }))
}

fn row(db: &Db, ws: &Workspace, side_tab_id: &str) -> Result<Option<Closed>> {
    let (Some((window_id, index)), Some(side)) = (place(ws, side_tab_id), ws.side_tab(side_tab_id))
    else {
        return Ok(None);
    };
    let tabs: Vec<Tab> = side.tabs.iter().filter_map(fresh_tab).collect();
    if tabs.is_empty() {
        return Ok(None);
    }
    Ok(Some(Closed::Row {
        window_id,
        index,
        side_tab: SideTab {
            tabs,
            ..side.clone()
        },
        context: ContextRows::take(db, side_tab_id)?,
    }))
}

fn place(ws: &Workspace, side_tab_id: &str) -> Option<(Id, usize)> {
    ws.windows.iter().find_map(|w| {
        w.side_tabs
            .iter()
            .position(|s| s.id == side_tab_id)
            .map(|i| (w.id.clone(), i))
    })
}

/// The tab as it reopens: its terminal panes only, each with no resume offer.
fn fresh_tab(tab: &Tab) -> Option<Tab> {
    let mut layout = tab.layout.clone();
    let others: Vec<Id> = layout
        .panes()
        .into_iter()
        .filter(|p| !layout.terminals().contains(p))
        .cloned()
        .collect();
    for pane in others {
        layout = layout.remove(&pane).ok().flatten()?;
    }
    for pane in layout.terminals().into_iter().cloned().collect::<Vec<_>>() {
        if let Some(Layout::Pane { resume, .. }) = layout.leaf_mut(&pane) {
            *resume = None;
        }
    }
    Some(Tab {
        layout,
        ..tab.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use crate::persist;
    use crate::state::{Core, Window};
    use crate::status::{Look, RuleKind};
    use crate::status_store;

    fn core() -> Core {
        let mut n = 0;
        Core::with_ids(
            Workspace::default(),
            Box::new(move || {
                n += 1;
                format!("id{n}")
            }),
        )
    }

    fn save(db: &Db, ws: &Workspace) {
        persist::save(
            db,
            &persist::Saved {
                workspace: ws.clone(),
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn reopen_restores_a_row_with_its_context() {
        let db = Db::open_in_memory().unwrap();
        let mut c = core();
        c.handle(Command::CreateWindow).unwrap();
        let window = c.workspace().windows[0].id.clone();
        c.handle(Command::CreateSideTab {
            window_id: window.clone(),
            index: None,
        })
        .unwrap();
        let ws = c.workspace().clone();
        let side = ws.windows[0].side_tabs[1].clone();
        save(&db, &ws);
        context::put_saved(&db, &side.id, "ticket", "PROJ-482", "manual", None, false).unwrap();
        let look = Look {
            label: "CI red".into(),
            accent: None,
            background: None,
        };
        let status = status_store::add_status(&db, Some(&side.id), &look).unwrap();
        let kind = RuleKind::ExitCode {
            pattern: "npm test".into(),
        };
        status_store::add_rule(&db, &status.id, &kind).unwrap();

        let close = Command::CloseSideTab {
            side_tab_id: side.id.clone(),
        };
        let closed = closing(&db, c.workspace(), &close).unwrap().unwrap();
        c.handle(close).unwrap();
        save(&db, c.workspace());
        assert!(context::saved_variables(&db, &side.id).unwrap().is_empty());

        let entry = Entry {
            closed,
            cwds: HashMap::new(),
        };
        c.handle(entry.reopen(c.workspace(), |_| true, &window))
            .unwrap();
        save(&db, c.workspace());
        let Closed::Row { context, .. } = &entry.closed else {
            panic!("a row closed");
        };
        context.restore(&db).unwrap();

        let back = &c.workspace().windows[0].side_tabs[1];
        assert_eq!(back, &side, "the row comes back as it was, at its place");
        let vars = context::saved_variables(&db, &side.id).unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].value.as_deref(), Some("PROJ-482"));
        let rules = status_store::rules(&db).unwrap();
        assert!(rules.iter().any(|r| r.status_id == status.id));
    }

    #[test]
    fn a_closed_tab_goes_back_to_its_row_else_a_new_row() {
        let db = Db::open_in_memory().unwrap();
        let mut c = core();
        c.handle(Command::CreateWindow).unwrap();
        let window = c.workspace().windows[0].id.clone();
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        c.handle(Command::CreateTab {
            side_tab_id: side.clone(),
        })
        .unwrap();
        let first = c.workspace().side_tab(&side).unwrap().tabs[0].clone();
        let close = Command::CloseTab {
            tab_id: first.id.clone(),
        };
        let entry = Entry {
            closed: closing(&db, c.workspace(), &close).unwrap().unwrap(),
            cwds: HashMap::new(),
        };
        assert!(matches!(entry.closed, Closed::Tab { index: 0, .. }));
        c.handle(close).unwrap();
        c.handle(entry.reopen(c.workspace(), |_| true, &window))
            .unwrap();
        assert_eq!(c.workspace().side_tab(&side).unwrap().tabs[0], first);

        // Its row is gone: the tab comes back as a new row at the window's end.
        c.handle(Command::CloseTab {
            tab_id: first.id.clone(),
        })
        .unwrap();
        c.handle(Command::CloseSideTab {
            side_tab_id: side.clone(),
        })
        .unwrap();
        c.handle(entry.reopen(c.workspace(), |_| true, &window))
            .unwrap();
        let w: &Window = &c.workspace().windows[0];
        assert_eq!(w.side_tabs.last().unwrap().tabs, vec![first]);
    }

    #[test]
    fn a_closed_window_without_a_view_falls_back() {
        let db = Db::open_in_memory().unwrap();
        let mut c = core();
        c.handle(Command::CreateWindow).unwrap();
        c.handle(Command::CreateWindow).unwrap();
        let (home, other) = (
            c.workspace().windows[0].id.clone(),
            c.workspace().windows[1].id.clone(),
        );
        let side = c.workspace().windows[0].side_tabs[0].id.clone();
        c.handle(Command::CreateTab {
            side_tab_id: side.clone(),
        })
        .unwrap();
        let tab = c.workspace().side_tab(&side).unwrap().tabs[0].id.clone();
        let close = Command::CloseTab { tab_id: tab };
        let entry = Entry {
            closed: closing(&db, c.workspace(), &close).unwrap().unwrap(),
            cwds: HashMap::new(),
        };
        let window_of = |cmd: Command| match cmd {
            Command::ReopenTab { window_id, .. } => window_id,
            other => panic!("a tab reopens, got {other:?}"),
        };
        // Listed but no view: the key's window gets it.
        let got = window_of(entry.reopen(c.workspace(), |id| id != &home, &other));
        assert_eq!(got, other);
        let got = window_of(entry.reopen(c.workspace(), |_| true, &other));
        assert_eq!(got, home);
    }

    #[test]
    fn the_list_keeps_ten() {
        let mut list = ClosedList::default();
        let entry = |i: usize| Entry {
            closed: Closed::Row {
                window_id: "w".into(),
                index: 0,
                side_tab: SideTab {
                    folder: false,
                    id: format!("s{i}"),
                    title: None,
                    tabs: vec![],
                    status: None,
                    icon: None,
                    accent: None,
                    name: None,
                },
                context: ContextRows::default(),
            },
            cwds: HashMap::new(),
        };
        for i in 0..KEEP {
            let t = list.push(entry(i));
            assert!(list.settle(t, true).is_empty());
        }
        // Parked while its close runs, so the close keeps its variables, but not reopened yet.
        let t10 = list.push(entry(KEEP));
        assert!(list.is_parked("s0") && list.is_parked("s10"));
        let newest = list.pop().unwrap();
        assert_eq!(newest.key(), "s9");
        // A failed reopen puts it back, newest again and still parked; a second failure drops it.
        assert!(list.put_back(newest).is_none());
        assert!(list.is_parked("s9"));
        let again = list.pop().unwrap();
        let dropped = list.put_back(again).unwrap();
        assert_eq!(dropped.key(), "s9");
        let s8 = list.pop().unwrap();
        assert_eq!(s8.key(), "s8");
        list.done.push_back(s8);
        list.done.push_back(dropped);
        let out = list.settle(t10, true);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].parked_side_tab().unwrap(), "s0");
        assert!(!list.is_parked("s0") && list.is_parked("s1"));
        // A success in between clears the count: only two failures in a row drop an entry.
        let s7 = list.pop().unwrap();
        assert!(list.put_back(s7).is_none());
        let s7 = list.pop().unwrap();
        list.reopened();
        assert!(list.put_back(s7).is_none());
        let s7 = list.pop().unwrap();
        list.done.push_back(s7);
        // A failed close drops its own entry, even beside another close of the same row.
        let failed = list.push(entry(11));
        let again = list.push(entry(11));
        let other = list.push(entry(12));
        assert!(list.settle(failed, false).is_empty());
        assert!(list.is_parked("s11") && list.is_parked("s12"));
        assert_eq!(list.settle(again, true)[0].key(), "s1");
        assert_eq!(list.settle(other, true)[0].key(), "s2");
        assert_eq!(list.pop().unwrap().key(), "s12");
        assert_eq!(list.pop().unwrap().key(), "s11");
    }
}
