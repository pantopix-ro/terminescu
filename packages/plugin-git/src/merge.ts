// The diff tab's editors (add-git-plugin D6): the text pane's own editor, inside a merge view.
// The working side keeps every extension the pane gives it, so saving and line endings work
// as in a text tab.

import { MergeView, unifiedMergeView } from "@codemirror/merge";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { editorTheme, type MountEditor } from "@terminescu/ui";

/** Side by side: the base on the left, read-only; the working copy on the right. */
export function sideBySide(original: string): MountEditor {
  return (parent, config) => {
    const merge = new MergeView({
      parent,
      a: { doc: original, extensions: [lineNumbers(), editorTheme, EditorView.lineWrapping, EditorState.readOnly.of(true), EditorView.editable.of(false)] },
      b: config,
      gutter: true,
      highlightChanges: true,
    });
    // No `reset`: new text from the core builds the view again around it.
    return { view: merge.b, destroy: () => merge.destroy() };
  };
}

/** Unified: one editor, the removed lines shown above the lines that replaced them. */
export function unified(original: string): MountEditor {
  const withDiff = (extensions: Extension): Extension => [extensions, unifiedMergeView({ original, mergeControls: false, gutter: true })];
  return (parent, config) => {
    const view = new EditorView({ parent, state: EditorState.create({ doc: config.doc, extensions: withDiff(config.extensions) }) });
    return {
      view,
      destroy: () => view.destroy(),
      reset: (c) => view.setState(EditorState.create({ doc: c.doc, extensions: withDiff(c.extensions) })),
    };
  };
}
