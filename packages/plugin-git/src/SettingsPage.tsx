// Settings › Git (add-git-plugin 3.5): where worktrees go, the diff layout and the GitHub switch.
// The folder and the switch live in the Rust half's plugin settings; the layout stays per Mac in
// the same key the diff tab's button and ⌥⌘U use.

import { useEffect, useState } from "react";
import type { SettingsPageProps } from "@terminescu/plugin-api";
import { loadMode, MODE_EVENT, saveMode, type DiffMode } from "./mode";

/** The Rust half's `settings` answer. */
export interface GitSettings {
  worktreeFolder: string | null;
  github: boolean;
  githubOn: boolean;
  githubOff: string | null;
  version: string | null;
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

const LAYOUTS: readonly { mode: DiffMode; label: string; hint: string }[] = [
  { mode: "side", label: "Side by side", hint: "HEAD on the left, read-only; the file on the right" },
  { mode: "unified", label: "Unified", hint: "one editor, removed lines above the lines that replaced them" },
];

export function SettingsPage({ bridge }: SettingsPageProps) {
  const [settings, setSettings] = useState<GitSettings>();
  const [folder, setFolder] = useState("");
  const [error, setError] = useState<string>();
  const [mode, setMode] = useState<DiffMode>(() => loadMode());

  useEffect(() => {
    let gone = false;
    bridge.call<GitSettings>("settings").then(
      (s) => {
        if (gone) return;
        setSettings(s);
        setFolder(s.worktreeFolder ?? "");
      },
      (e: unknown) => !gone && setError(message(e)),
    );
    const off = bridge.on("github", () => void bridge.call<GitSettings>("settings").then((s) => !gone && setSettings(s), () => undefined));
    return () => {
      gone = true;
      off();
    };
  }, [bridge]);

  useEffect(() => {
    const follow = (e: Event) => setMode((e as CustomEvent<DiffMode>).detail);
    window.addEventListener(MODE_EVENT, follow);
    return () => window.removeEventListener(MODE_EVENT, follow);
  }, []);

  const save = (key: string, value: unknown) =>
    bridge.call<GitSettings>("setSetting", { key, value }).then(
      (s) => {
        setError(undefined);
        setSettings(s);
        setFolder(s.worktreeFolder ?? "");
      },
      (e: unknown) => setError(message(e)),
    );
  const commitFolder = () => {
    if (settings && folder.trim() !== (settings.worktreeFolder ?? "")) void save("worktreeFolder", folder);
  };

  if (!settings) return error ? <p className="small app-error git-settings-error">{error}</p> : null;
  return (
    <section aria-label="Git settings" className="git-settings">
      <div className="srow">
        <span>Worktree folder</span>
        <input
          className="ao-input"
          aria-label="Worktree folder"
          placeholder="next to the repository"
          spellCheck={false}
          value={folder}
          onChange={(e) => setFolder(e.target.value)}
          onBlur={commitFolder}
          onKeyDown={(e) => e.key === "Enter" && commitFolder()}
        />
        <span className="small ao-dim grow">
          {folder.trim() ? `${folder.trim().replace(/\/+$/, "")}/<repo>/<branch>` : "<repo>-<branch> beside the repository"}
        </span>
      </div>
      {error && (
        <p className="small app-error git-settings-error" role="alert">
          {error}
        </p>
      )}
      <div role="radiogroup" aria-label="Diff layout">
        {LAYOUTS.map((l) => (
          <label key={l.mode} className="srow">
            <input type="radio" name="git-diff-mode" checked={mode === l.mode} onChange={() => saveMode(l.mode)} />
            <span>{l.label}</span>
            <span className="small ao-dim grow">{l.hint}</span>
          </label>
        ))}
      </div>
      <label className="srow">
        <input type="checkbox" aria-label="GitHub through gh" checked={settings.github} onChange={(e) => void save("github", e.target.checked)} />
        <span>GitHub through gh</span>
        <span className="small ao-dim grow git-github-state">
          {settings.githubOn ? "on: pull request chips and items" : `off: ${settings.githubOff ?? "gh is not available"}`}
        </span>
      </label>
      <p className="small ao-dim">gh keeps its own login. Terminescu never reads or stores a GitHub token.</p>
    </section>
  );
}
