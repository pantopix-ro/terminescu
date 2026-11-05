import type { Completion, CompletionContext, CompletionResult } from "@codemirror/autocomplete";
import { autocompletion } from "@codemirror/autocomplete";
import { RangeSetBuilder, StateEffect, type Extension } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

/** What the editor knows of a side tab's variables: never a value (D29, D35). */
export interface VarInfo {
  name: string;
  mode: string;
}

export type RefKind = "known" | "secret" | "unknown";

export interface VarRef {
  from: number;
  to: number;
  name: string;
  kind: RefKind;
}

/** `{{pins}}` is the side tab's pins when no variable takes the name. */
const BUILT_IN = new Set(["pins"]);

/** Every `{{name}}` in `text`, with whether the context has it. */
export function varRefs(text: string, vars: readonly VarInfo[]): VarRef[] {
  const modes = new Map(vars.map((v) => [v.name, v.mode]));
  const out: VarRef[] = [];
  for (const m of text.matchAll(/\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g)) {
    const name = m[1]!;
    const mode = modes.get(name);
    const kind: RefKind = mode === "secret" ? "secret" : mode !== undefined || BUILT_IN.has(name) ? "known" : "unknown";
    out.push({ from: m.index!, to: m.index! + m[0].length, name, kind });
  }
  return out;
}

/**
 * What autocomplete offers after `{{`: each variable as `{{name}}`, a secret only as `$NAME`
 * so its value never lands in the text (D35), then `{{pins}}`.
 */
export function varOptions(vars: readonly VarInfo[]): { label: string; apply: string; detail: string }[] {
  const out = vars.map((v) =>
    v.mode === "secret"
      ? { label: `$${v.name}`, apply: `$${v.name}`, detail: "secret, from the environment" }
      : { label: `{{${v.name}}}`, apply: `{{${v.name}}}`, detail: v.mode },
  );
  if (!vars.some((v) => v.name === "pins")) out.push({ label: "{{pins}}", apply: "{{pins}}", detail: "the side tab's pins" });
  return out;
}

/** Ask the chips to read the variables again. */
const refresh = StateEffect.define<null>();

const MARK: Record<RefKind, Decoration> = {
  known: Decoration.mark({ class: "cm-var-chip" }),
  secret: Decoration.mark({ class: "cm-var-chip cm-var-secret", attributes: { title: "secret: inserts as $NAME" } }),
  unknown: Decoration.mark({ class: "cm-var-unknown", attributes: { title: "unknown variable" } }),
};

/**
 * `{{variable}}` chips over the plain text, so the file stays byte-exact (D29): a known name
 * shows as a chip, an unknown one gets a wavy underline titled "unknown variable".
 * `get` answers the side tab's variables now and whether chips apply to this document.
 */
export function varChips(get: () => { on: boolean; vars: readonly VarInfo[] }): Extension {
  const build = (view: EditorView): DecorationSet => {
    const { on, vars } = get();
    const b = new RangeSetBuilder<Decoration>();
    if (!on) return b.finish();
    for (const { from, to } of view.visibleRanges) {
      // A chip can straddle the edge of a visible range; read a little around it.
      const start = Math.max(0, from - 200);
      const text = view.state.sliceDoc(start, Math.min(view.state.doc.length, to + 200));
      for (const r of varRefs(text, vars)) {
        const a = start + r.from;
        if (a + (r.to - r.from) < from || a > to) continue;
        b.add(a, start + r.to, MARK[r.kind]);
      }
    }
    return b.finish();
  };
  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = build(view);
      }
      update(u: ViewUpdate) {
        if (u.docChanged || u.viewportChanged || u.transactions.some((t) => t.effects.some((e) => e.is(refresh)))) {
          this.decorations = build(u.view);
        }
      }
    },
    { decorations: (p) => p.decorations },
  );
  const source = (ctx: CompletionContext): CompletionResult | null => {
    const { on, vars } = get();
    if (!on) return null;
    const before = ctx.matchBefore(/\{\{\s*[A-Za-z0-9_]*$/);
    if (!before) return null;
    // closeBrackets may already have typed the closing braces.
    const after = ctx.state.sliceDoc(ctx.pos, ctx.pos + 2);
    const to = after === "}}" ? ctx.pos + 2 : ctx.pos;
    const options: Completion[] = varOptions(vars).map((o) => ({ label: o.label, apply: o.apply, detail: o.detail, type: "variable" }));
    return { from: before.from, to, options, filter: false };
  };
  return [plugin, autocompletion({ override: [source], icons: false })];
}

/** Tells the chips in `view` that the variables changed. */
export function refreshChips(view: EditorView) {
  view.dispatch({ effects: refresh.of(null) });
}
