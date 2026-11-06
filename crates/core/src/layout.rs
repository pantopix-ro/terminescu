//! A tab's split tree (D3). Leaves are panes; a split lays its children out in a row (side by
//! side) or a column (stacked), with sizes in thousandths that always add up to [`TOTAL`].
//!
//! Splitting a pane in the direction of its parent adds a sibling, like iTerm; in the other
//! direction it wraps the pane in a new split. Removing a pane gives its room to a neighbour and
//! collapses a split left with one child, so the tree never holds a split of one or a split
//! inside a split of the same direction.

use serde::{Deserialize, Serialize};

pub type Id = String;

/// What the sizes of one split add up to.
pub const TOTAL: u16 = 1000;
/// The smallest share a pane keeps when a split resizes.
pub const MIN_SIZE: u16 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    /// Side by side; `⌘D` splits right.
    Row,
    /// Stacked; `⌘⇧D` splits down.
    Column,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum Layout {
    Pane {
        id: Id,
        /// A Claude session that ran here before a reboot; the pane offers to resume it (D23).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume: Option<String>,
        /// Set on a transcript pane: the Claude session it mirrors, read-only (task 4.4). Such
        /// a pane has no shell.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transcript: Option<String>,
        /// Set on a text pane: the document it edits (D28). Such a pane has no shell either.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        doc: Option<Id>,
    },
    Split {
        id: Id,
        dir: Dir,
        children: Vec<Layout>,
        sizes: Vec<u16>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    UnknownPane,
    UnknownSplit,
    BadSizes,
    /// Splitting would leave a share under [`MIN_SIZE`].
    TooSmall,
}

impl Layout {
    pub fn pane(id: impl Into<Id>) -> Self {
        Layout::Pane {
            id: id.into(),
            resume: None,
            transcript: None,
            doc: None,
        }
    }

    /// A text pane editing document `doc` (D28).
    pub fn text(id: impl Into<Id>, doc: impl Into<Id>) -> Self {
        Layout::Pane {
            id: id.into(),
            resume: None,
            transcript: None,
            doc: Some(doc.into()),
        }
    }

    /// A transcript pane mirroring `session` (task 4.4).
    pub fn transcript(id: impl Into<Id>, session: impl Into<String>) -> Self {
        Layout::Pane {
            id: id.into(),
            resume: None,
            transcript: Some(session.into()),
            doc: None,
        }
    }

    /// The panes that run a shell, in reading order: every pane but transcript and text panes.
    pub fn terminals(&self) -> Vec<&Id> {
        let mut out = Vec::new();
        self.walk(&mut |l| {
            if let Layout::Pane {
                id,
                transcript: None,
                doc: None,
                ..
            } = l
            {
                out.push(id);
            }
        });
        out
    }

    /// Text panes and their documents, in reading order.
    pub fn docs(&self) -> Vec<(&Id, &Id)> {
        let mut out = Vec::new();
        self.walk(&mut |l| {
            if let Layout::Pane {
                id, doc: Some(d), ..
            } = l
            {
                out.push((id, d));
            }
        });
        out
    }

    /// A tab with only transcript panes left has nothing of its own to show; it closes.
    pub fn holds_work(&self) -> bool {
        !self.terminals().is_empty() || !self.docs().is_empty()
    }

    /// Pane ids, in reading order.
    pub fn panes(&self) -> Vec<&Id> {
        let mut out = Vec::new();
        self.walk(&mut |l| {
            if let Layout::Pane { id, .. } = l {
                out.push(id);
            }
        });
        out
    }

    fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Layout)) {
        f(self);
        if let Layout::Split { children, .. } = self {
            for c in children {
                c.walk(f);
            }
        }
    }

    pub fn has_pane(&self, pane: &str) -> bool {
        self.panes().iter().any(|p| *p == pane)
    }

    pub fn has_split(&self, split: &str) -> bool {
        let mut found = false;
        self.walk(&mut |l| {
            if let Layout::Split { id, .. } = l {
                found |= id == split;
            }
        });
        found
    }

    /// The pane leaf with this id.
    pub fn leaf(&self, pane: &str) -> Option<&Layout> {
        match self {
            Layout::Pane { id, .. } if id == pane => Some(self),
            Layout::Pane { .. } => None,
            Layout::Split { children, .. } => children.iter().find_map(|c| c.leaf(pane)),
        }
    }

    /// The pane leaf with this id, to change.
    pub fn leaf_mut(&mut self, pane: &str) -> Option<&mut Layout> {
        match self {
            Layout::Pane { id, .. } if id == pane => Some(self),
            Layout::Pane { .. } => None,
            Layout::Split { children, .. } => children.iter_mut().find_map(|c| c.leaf_mut(pane)),
        }
    }

    /// The resume offer of a pane, if any.
    pub fn resume_of(&self, pane: &str) -> Option<&str> {
        let mut out = None;
        self.walk(&mut |l| {
            if let Layout::Pane { id, resume, .. } = l
                && id == pane
            {
                out = resume.as_deref();
            }
        });
        out
    }

    /// A copy with `new` placed after `target`. `split_id` names the split when one is needed.
    pub fn split(
        &self,
        target: &str,
        dir: Dir,
        new: Layout,
        split_id: Id,
    ) -> Result<Self, LayoutError> {
        self.split_at(target, dir, false, new, split_id)
    }

    /// A copy with `new`, a pane or a whole tab's layout, beside `target`: before it (left or
    /// up) or after it (right or down), taking half of its share (task 8.32). A split `new`
    /// in the same direction as its new parent merges into it.
    pub fn split_at(
        &self,
        target: &str,
        dir: Dir,
        before: bool,
        new: Layout,
        split_id: Id,
    ) -> Result<Self, LayoutError> {
        let mut out = self.clone();
        let nested = matches!(new, Layout::Split { .. });
        let mut new = Some(new);
        let mut too_small = false;
        let place = Place {
            target,
            dir,
            before,
            split_id: &split_id,
        };
        if !split_in(&mut out, &place, &mut new, &mut too_small) {
            return Err(if too_small {
                LayoutError::TooSmall
            } else {
                LayoutError::UnknownPane
            });
        }
        if nested {
            out.normalize();
            // Merged into its new parent, a docked tab's panes share half a pane.
            if !out.shares_fit() {
                return Err(LayoutError::TooSmall);
            }
        }
        Ok(out)
    }

    /// A copy without `target`; `None` when it was the last pane.
    pub fn remove(&self, target: &str) -> Result<Option<Self>, LayoutError> {
        if !self.has_pane(target) {
            return Err(LayoutError::UnknownPane);
        }
        let mut out = self.clone();
        if !remove_in(&mut out, target) {
            return Ok(None);
        }
        out.normalize();
        Ok(Some(out))
    }

    /// A copy with new sizes for one split. Sizes scale to [`TOTAL`]; none may end up under
    /// [`MIN_SIZE`].
    pub fn resize(&self, split: &str, sizes: &[u16]) -> Result<Self, LayoutError> {
        let mut out = self.clone();
        let node = out.split_mut(split).ok_or(LayoutError::UnknownSplit)?;
        let Layout::Split {
            children,
            sizes: current,
            ..
        } = node
        else {
            unreachable!("split_mut returns splits");
        };
        if sizes.len() != children.len() {
            return Err(LayoutError::BadSizes);
        }
        let scaled = rescale(sizes.iter().map(|&s| u32::from(s)).collect());
        if scaled.iter().any(|&s| s < MIN_SIZE) {
            return Err(LayoutError::BadSizes);
        }
        *current = scaled;
        Ok(out)
    }

    fn split_mut(&mut self, split: &str) -> Option<&mut Layout> {
        match self {
            Layout::Split { id, .. } if id == split => Some(self),
            Layout::Split { children, .. } => children.iter_mut().find_map(|c| c.split_mut(split)),
            Layout::Pane { .. } => None,
        }
    }

    /// Every share at least [`MIN_SIZE`].
    fn shares_fit(&self) -> bool {
        match self {
            Layout::Pane { .. } => true,
            Layout::Split {
                children, sizes, ..
            } => sizes.iter().all(|&s| s >= MIN_SIZE) && children.iter().all(Layout::shares_fit),
        }
    }

    /// Collapses splits of one child and merges a split into a parent of the same direction.
    fn normalize(&mut self) {
        let Layout::Split {
            dir,
            children,
            sizes,
            ..
        } = self
        else {
            return;
        };
        for c in children.iter_mut() {
            c.normalize();
        }
        let mut flat_children = Vec::new();
        let mut flat_sizes: Vec<u32> = Vec::new();
        for (c, &s) in children.drain(..).zip(sizes.iter()) {
            match c {
                Layout::Split {
                    dir: d,
                    children: gc,
                    sizes: gs,
                    ..
                } if d == *dir => {
                    for (g, gs) in gc.into_iter().zip(gs) {
                        flat_children.push(g);
                        flat_sizes.push(u32::from(s) * u32::from(gs));
                    }
                }
                c => {
                    flat_children.push(c);
                    flat_sizes.push(u32::from(s) * u32::from(TOTAL));
                }
            }
        }
        *children = flat_children;
        *sizes = rescale(flat_sizes);
        if children.len() == 1 {
            *self = children.pop().expect("one child");
        }
    }
}

/// Where [`split_in`] puts the new layout.
struct Place<'a> {
    target: &'a str,
    dir: Dir,
    before: bool,
    split_id: &'a str,
}

fn split_in(
    node: &mut Layout,
    place: &Place<'_>,
    new: &mut Option<Layout>,
    too_small: &mut bool,
) -> bool {
    let Place {
        target,
        dir,
        before,
        split_id,
    } = *place;
    match node {
        Layout::Pane { id, .. } => {
            if id != target {
                return false;
            }
            let old = std::mem::replace(node, Layout::pane(""));
            let new = new.take().expect("placed once");
            *node = Layout::Split {
                id: split_id.into(),
                dir,
                children: if before {
                    vec![new, old]
                } else {
                    vec![old, new]
                },
                sizes: if before {
                    vec![TOTAL / 2, TOTAL - TOTAL / 2]
                } else {
                    vec![TOTAL - TOTAL / 2, TOTAL / 2]
                },
            };
            true
        }
        Layout::Split {
            dir: d,
            children,
            sizes,
            ..
        } => {
            let hit = children
                .iter()
                .position(|c| matches!(c, Layout::Pane { id, .. } if id == target));
            if let Some(i) = hit
                && *d == dir
            {
                if sizes[i] < 2 * MIN_SIZE {
                    *too_small = true;
                    return false;
                }
                let half = sizes[i] / 2;
                sizes[i] -= half;
                let at = if before { i } else { i + 1 };
                sizes.insert(at, half);
                children.insert(at, new.take().expect("placed once"));
                return true;
            }
            children
                .iter_mut()
                .any(|c| split_in(c, place, new, too_small))
        }
    }
}

/// Removes `target` below `node`. Returns false when `node` itself is the target leaf.
fn remove_in(node: &mut Layout, target: &str) -> bool {
    match node {
        Layout::Pane { id, .. } => id != target,
        Layout::Split {
            children, sizes, ..
        } => {
            if let Some(i) = children
                .iter()
                .position(|c| matches!(c, Layout::Pane { id, .. } if id == target))
            {
                children.remove(i);
                let freed = sizes.remove(i);
                let heir = i.saturating_sub(1).min(sizes.len() - 1);
                sizes[heir] += freed;
                return true;
            }
            for c in children.iter_mut() {
                if c.has_pane(target) {
                    return remove_in(c, target);
                }
            }
            true
        }
    }
}

/// Scales weights to sizes that add up to exactly [`TOTAL`], each at least 1.
fn rescale(weights: Vec<u32>) -> Vec<u16> {
    let n = weights.len().max(1) as u64;
    let sum: u64 = weights.iter().map(|&w| u64::from(w)).sum();
    let mut out: Vec<u16> = if sum == 0 {
        vec![(u64::from(TOTAL) / n) as u16; weights.len()]
    } else {
        weights
            .iter()
            .map(|&w| ((u64::from(w) * u64::from(TOTAL) / sum) as u16).max(1))
            .collect()
    };
    let total: i32 = out.iter().map(|&s| i32::from(s)).sum();
    let diff = i32::from(TOTAL) - total;
    // Hand the rounding difference to the largest share, so no share drops under 1.
    if let Some(big) = (0..out.len()).max_by_key(|&i| out[i]) {
        out[big] = (i32::from(out[big]) + diff) as u16;
    }
    out
}
