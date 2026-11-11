## Context

add-git-plugin D1 to D7 stand. harden-plugin-host gave `SideTabClosed` the plugin's row data,
which D6 here builds on.

## Decisions

### D1. The tick reads no git

`prtick` looks up the pull request of the lead pane's cached facts, `due` set. A pane whose facts
hold no branch drops its tick; the next refresh with a branch starts it again through `look_up`.
Rejected: keeping the full refresh. Facts already refresh on every command end.

### D2. Decorations as a diff

The plugin keeps what it last published per tab: the branch chip, the PR chip and the three
facts. A new publish sets only what differs. A chip that goes away clears the tab and sets the
one that stays. Rejected: a remove-by-key call in `PluginCx`; it widens the API for one caller.

### D3. The facts event names its root

`facts` carries `{ pane, root }`, `root` null outside a repository. A diff tab reads again only
for its own root. The Changes section keeps listening by pane.

### D4. A diff tab keeps its place

The diff mounts register a live handle with DiffPane. A new base text with the same layout goes
into the open editor: `merge.a` gets the change in side-by-side, `updateOriginalDoc` in unified.
A layout switch builds the editor again and puts the scroll top back. `sideBySide` gains `reset`,
which writes only the changed middle of the text, so a reload from disk keeps the place too.
Rejected: a scroll restore alone; the editor would still flash.

### D5. Bare layouts

`main_root` is the shared git folder when it is not named `.git`: git runs there for worktree
lists and removes. `repo_name` reads `shop` from `shop.git` and the parent folder from `.bare`.
`row_worktree` answers `changed: null` when the repository is unreadable; it never fails, so a
close never blocks on it.

### D6. Orphan notice

`side_tab_closed` reads the mark from the event data. When it holds one and no ask ran for the
row, the plugin emits `orphan { worktree, repo, branch }`. The UI half notes the rows it asked
about, so a close that asked does not raise the notice. The notice offers Remove and Keep.
Remove calls `worktreeRemoveAt`, which checks the worktree against git like `worktreeRemove` and
never forces; a worktree with changes asks again before forcing.

### D7. Renames and deletes

`baseText` takes an optional `from`, the path at HEAD. A deleted file (the Changes row is
unstaged and gone from disk, or the Claude change says `deleted`) opens with `deleted: true` in
the view source. DiffPane then shows "Deleted: the file is gone from the working tree." and
mounts the working side read-only.

### D8. Small ones

`gh auth status --hostname github.com`. The worktree folder setting drops trailing slashes and
`.` parts before use. The New Row dialog asks `worktreeWhere` for its detail line. Diff colours
move into `diffTheme`, given to both merge editors.

## Risks / Trade-offs

- [Medium] D4 couples DiffPane to the mount closures through a mutable handle. Covered by unit
  tests on the handle and the git e2e.
- [Low] D2 caches per tab; a plugin restart clears the cache with the decorations, and
  `AppLaunched` rebuilds both.
- [Low] D6 notice may show for a row closed from another window's ask. Acceptable: Keep closes
  it.

## Review fixes

- A CRLF base in a unified diff is cut like the merge view's original, so the update fits; DiffPane rebuilds the editor when an in-place update throws.
- A `*.git` shared folder counts as bare only when `core.bare` says so (read from its config file, no extra git call).
- The "asked" mark for a closing worktree row lapses after 30 s, so a cancelled close no longer hides a later orphan notice.
