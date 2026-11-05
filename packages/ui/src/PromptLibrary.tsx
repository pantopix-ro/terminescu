import type { Host, Prompt, PromptVersion } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { timeAgo } from "./names";

type Api = Pick<
  Host,
  | "searchPrompts"
  | "createPrompt"
  | "savePrompt"
  | "updatePrompt"
  | "promptVersions"
  | "restorePrompt"
  | "duplicatePrompt"
  | "archivePrompt"
  | "onPrompts"
>;

export interface PromptLibraryProps {
  host: Api;
  /** The prompt to show first, by key. */
  selectKey?: string;
  onError(message: string): void;
  onClose(): void;
  /** Opens the prompt in a text tab, where `⌘S` saves a version (D14, D27). */
  onEditInText?(promptId: string): void;
}

const tagsOf = (text: string) =>
  text
    .split(/[,\s]+/)
    .map((t) => t.trim())
    .filter(Boolean);

function Detail({
  prompt,
  host,
  run,
  onSelect,
  onEditInText,
}: {
  prompt: Prompt;
  host: Api;
  run<T>(p: Promise<T>): Promise<T | undefined>;
  onSelect(id: string): void;
  onEditInText?(promptId: string): void;
}) {
  const [name, setName] = useState(prompt.name);
  const [tags, setTags] = useState(prompt.tags.join(", "));
  const [body, setBody] = useState(prompt.body);
  const [versions, setVersions] = useState<PromptVersion[]>([]);
  const [showVersions, setShowVersions] = useState(false);
  useEffect(() => {
    setName(prompt.name);
    setTags(prompt.tags.join(", "));
    setBody(prompt.body);
  }, [prompt.id, prompt.name, prompt.tags.join(","), prompt.body]);
  useEffect(() => {
    if (showVersions) void run(host.promptVersions(prompt.id)).then((v) => v && setVersions(v));
  }, [showVersions, prompt.id, prompt.version]);
  const metaChanged = name.trim() !== prompt.name || tagsOf(tags).join(",") !== prompt.tags.join(",");
  const saveMeta = () => metaChanged && void run(host.updatePrompt(prompt.id, name, tagsOf(tags)));
  const dirty = body !== prompt.body;

  return (
    <section className="pl-detail" aria-label={`Prompt ${prompt.name}`} data-prompt={prompt.key}>
      <div className="pl-row">
        <input className="ao-input grow" aria-label="Prompt name" value={name} onChange={(e) => setName(e.target.value)} onBlur={saveMeta} onKeyDown={(e) => e.key === "Enter" && saveMeta()} />
        <span className="small ao-dim pl-version">v{prompt.version}</span>
      </div>
      <input
        className="ao-input"
        aria-label="Tags"
        placeholder="tags, comma separated"
        value={tags}
        onChange={(e) => setTags(e.target.value)}
        onBlur={saveMeta}
        onKeyDown={(e) => e.key === "Enter" && saveMeta()}
      />
      <textarea
        className="ao-input pl-body"
        aria-label="Prompt body"
        spellCheck={false}
        value={body}
        onChange={(e) => setBody(e.target.value)}
        onKeyDown={(e) => {
          // Escape would close the sheet and drop the edit: it leaves the field first.
          if (e.key === "Escape" && dirty) {
            e.stopPropagation();
            e.currentTarget.blur();
          }
          if (e.metaKey && e.key === "s") {
            e.preventDefault();
            if (dirty) void run(host.savePrompt(prompt.id, body));
          }
        }}
      />
      <div className="pl-row">
        <span className="small ao-dim grow">
          {prompt.variables.length ? prompt.variables.map((v) => `{{${v}}}`).join(" ") : "no variables"}
        </span>
        {onEditInText && (
          <button type="button" className="ao-btn ghost" onClick={() => onEditInText(prompt.id)}>
            edit in a text tab
          </button>
        )}
        <button type="button" className="ao-btn ghost" onClick={() => setShowVersions((s) => !s)} aria-expanded={showVersions}>
          versions
        </button>
        <button type="button" className="ao-btn ghost" onClick={() => void run(host.duplicatePrompt(prompt.id)).then((p) => p && onSelect(p.id))}>
          duplicate
        </button>
        <button type="button" className="ao-btn ghost" onClick={() => void run(host.archivePrompt(prompt.id, !prompt.archived))}>
          {prompt.archived ? "unarchive" : "archive"}
        </button>
        <button type="button" className="ao-btn primary" disabled={!dirty} onClick={() => void run(host.savePrompt(prompt.id, body))}>
          Save ⌘S
        </button>
      </div>
      {showVersions && (
        <ul className="pl-versions" aria-label="Versions">
          {versions.map((v) => (
            <li key={v.version} className="pl-ver" data-version={v.version}>
              <span className="ao-pixel ao-dim">v{v.version}</span>
              <span className="small ao-dim">{timeAgo(v.createdAt)}</span>
              <span className="small pl-ver-body">{v.body.split("\n")[0]}</span>
              {v.version !== prompt.version && (
                <button type="button" className="link small" aria-label={`Restore version ${v.version}`} onClick={() => void run(host.restorePrompt(prompt.id, v.version))}>
                  restore
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function NewPromptForm({ host, run, onDone }: { host: Api; run<T>(p: Promise<T>): Promise<T | undefined>; onDone(id?: string): void }) {
  const [name, setName] = useState("");
  const [tags, setTags] = useState("");
  const [body, setBody] = useState("");
  return (
    <form
      className="pl-detail"
      aria-label="New prompt"
      onSubmit={(e) => {
        e.preventDefault();
        void run(host.createPrompt({ name, tags: tagsOf(tags), body })).then((p) => p && onDone(p.id));
      }}
    >
      <input className="ao-input" aria-label="Prompt name" placeholder="name" autoFocus value={name} onChange={(e) => setName(e.target.value)} />
      <input className="ao-input" aria-label="Tags" placeholder="tags, comma separated" value={tags} onChange={(e) => setTags(e.target.value)} />
      <textarea className="ao-input pl-body" aria-label="Prompt body" placeholder="Markdown with {{variables}}" spellCheck={false} value={body} onChange={(e) => setBody(e.target.value)} />
      <div className="pl-row">
        <span className="grow" />
        <button type="button" className="ao-btn ghost" onClick={() => onDone()}>
          cancel
        </button>
        <button type="submit" className="ao-btn primary">
          Create
        </button>
      </div>
    </form>
  );
}

/** The prompt library (D14): search, tags, versions with restore, duplicate, archive. */
export function PromptLibrary({ host, selectKey, onError, onClose, onEditInText }: PromptLibraryProps) {
  const [query, setQuery] = useState("");
  const [archived, setArchived] = useState(false);
  const [list, setList] = useState<Prompt[]>([]);
  const [selected, setSelected] = useState<string>();
  const [creating, setCreating] = useState(false);
  const [tick, setTick] = useState(0);
  const search = useRef<HTMLInputElement>(null);
  const frame = useRef<HTMLDivElement>(null);
  const run = <T,>(p: Promise<T>) =>
    p.catch((e: unknown) => {
      onError(String(e));
      return undefined;
    });

  useEffect(() => search.current?.focus(), []);
  // On the window, so Escape closes the sheet even after the focused control went away (an
  // archived or created prompt). The body field stops it while it holds unsaved text.
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      e.preventDefault();
      close.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  useEffect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    void host.onPrompts(() => setTick((t) => t + 1)).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host]);
  useEffect(() => {
    let stale = false;
    void run(host.searchPrompts(query, archived)).then((found) => {
      if (stale || !found) return;
      setList(found);
      setSelected((s) => {
        if (s && found.some((p) => p.id === s)) return s;
        return (selectKey && found.find((p) => p.key === selectKey)?.id) ?? found[0]?.id;
      });
    });
    return () => {
      stale = true;
    };
  }, [query, archived, tick]);
  const current = list.find((p) => p.id === selected);

  return (
    <div className="sheet-back" onPointerDown={onClose}>
      <div
        ref={frame}
        tabIndex={-1}
        className="ao-frame prompt-library"
        role="dialog"
        aria-labelledby="pl-t"
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div className="ao-frame-head">
          <i className="ao-i ao-i-dot" />
          <span id="pl-t" className="ao-pixel">
            Prompt library
          </span>
        </div>
        <div className="ao-frame-body pl-body-wrap">
          <aside className="pl-side">
            <input
              ref={search}
              className="ao-input"
              aria-label="Search prompts"
              placeholder="search, #tag"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <div className="pl-row">
              <label className="small ao-dim grow">
                <input type="checkbox" checked={archived} onChange={(e) => setArchived(e.target.checked)} /> archived
              </label>
              <button type="button" className="ao-btn ghost" onClick={() => setCreating(true)}>
                + prompt
              </button>
            </div>
            <ul className="pl-list" aria-label="Prompts">
              {list.map((p) => (
                <li key={p.id}>
                  <button
                    type="button"
                    className={`pl-item${p.id === selected && !creating ? " sel" : ""}`}
                    data-prompt={p.key}
                    aria-current={p.id === selected && !creating}
                    onClick={() => {
                      setCreating(false);
                      setSelected(p.id);
                    }}
                  >
                    <span className="pl-name">{p.name}</span>
                    <span className="small ao-dim">
                      {p.tags.map((t) => `#${t}`).join(" ")}
                      {p.useCount ? ` · used ${p.useCount}× · ${timeAgo(p.lastUsedAt!)}` : ""}
                    </span>
                  </button>
                </li>
              ))}
              {list.length === 0 && <li className="small ao-dim">{query ? "Nothing matches." : "No prompts."}</li>}
            </ul>
          </aside>
          {creating ? (
            <NewPromptForm
              host={host}
              run={run}
              onDone={(id) => {
                setCreating(false);
                // The form leaves with the focus; keep it in the sheet so Escape still closes it.
                frame.current?.focus();
                if (id) {
                  setQuery("");
                  setArchived(false);
                  setSelected(id);
                }
              }}
            />
          ) : current ? (
            <Detail key={current.id} prompt={current} host={host} run={run} onSelect={setSelected} onEditInText={onEditInText} />
          ) : (
            <p className="small ao-dim pl-detail">Pick a prompt, or add one.</p>
          )}
        </div>
      </div>
    </div>
  );
}
