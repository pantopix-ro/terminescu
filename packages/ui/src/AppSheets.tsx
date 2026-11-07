// The public release's sheets (add-public-release): first run, About, Licenses, Privacy,
// Feedback and Uninstall. Each opens over the window like the other sheets; Esc closes it.

import { MAKER, MAKER_URL, PUBLIC_REPO, type AppInfo, type FeedbackRoute, type Host } from "@terminescu/host";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { NOT_AFFILIATED, PRIVACY_POINTS } from "./privacy";

type Api = Pick<Host, "appInfo" | "feedbackOpen" | "uninstallHelpers" | "appQuit" | "openUrl" | "settings" | "setSettings">;

function Sheet({ label, className = "", onClose, children }: { label: string; className?: string; onClose(): void; children: ReactNode }) {
  const frame = useRef<HTMLDivElement>(null);
  useEffect(() => frame.current?.focus(), []);
  return (
    <div className="sheet-back" onPointerDown={onClose}>
      <div
        ref={frame}
        tabIndex={-1}
        className={`ao-frame app-sheet ${className}`}
        role="dialog"
        aria-label={label}
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      >
        <div className="ao-frame-body">{children}</div>
      </div>
    </div>
  );
}

function useInfo(host: Api, onError: (m: string) => void) {
  const [info, setInfo] = useState<AppInfo>();
  useEffect(() => {
    void host.appInfo().then(setInfo, (e: unknown) => onError(String(e)));
  }, [host, onError]);
  return info;
}

export interface AppSheetProps {
  host: Api;
  onError(message: string): void;
  onClose(): void;
}

/** Terminescu › About Terminescu (D7). */
export function AboutSheet({ host, onError, onClose, onLicenses }: AppSheetProps & { onLicenses(): void }) {
  const info = useInfo(host, onError);
  const link = (url: string) => () => void host.openUrl(url).catch((e: unknown) => onError(String(e)));
  return (
    <Sheet label="About Terminescu" className="about-sheet" onClose={onClose}>
      <h2 className="ao-pixel">TERMINESCU</h2>
      <p className="about-version">{info ? `Version ${info.version} (${info.build})` : " "}</p>
      <p>
        Made by{" "}
        <button type="button" className="link" onClick={link(MAKER_URL)}>
          {MAKER}
        </button>{" "}
        · <span className="small ao-dim">pantopix.ro</span>
      </p>
      <p className="small">
        Functional Source License 1.1, Apache 2.0 future license (FSL-1.1-ALv2).{" "}
        <button type="button" className="link small" onClick={link(`https://github.com/${PUBLIC_REPO}/blob/main/LICENSE.md`)}>
          Read the license
        </button>
      </p>
      <p className="small ao-dim">{NOT_AFFILIATED}</p>
      <div className="srow">
        <span className="grow" />
        <button type="button" className="ao-btn ghost" onClick={onLicenses}>
          Licenses
        </button>
        <button type="button" className="ao-btn primary" onClick={onClose}>
          Close
        </button>
      </div>
    </Sheet>
  );
}

interface Credits {
  rust: boolean;
  groups: { license: string; packages: string[]; text: string }[];
}

/** Help › Licenses (D8): the build's generated credits, from licenses.json. */
export function LicensesSheet({ onClose }: { onClose(): void }) {
  const [credits, setCredits] = useState<Credits | null>();
  const [query, setQuery] = useState("");
  useEffect(() => {
    fetch("licenses.json")
      .then((r) => (r.ok ? (r.json() as Promise<Credits>) : null))
      .then(setCredits, () => setCredits(null));
  }, []);
  const q = query.trim().toLowerCase();
  const shown = credits?.groups.filter((g) => !q || g.license.toLowerCase().includes(q) || g.packages.some((p) => p.toLowerCase().includes(q)));
  return (
    <Sheet label="Licenses" className="licenses-sheet" onClose={onClose}>
      <h3 className="ao-pixel ao-dim">Licenses</h3>
      <p className="small">Terminescu is built on these open-source libraries. Each is listed with its license and the notices it asks to keep.</p>
      <input className="ao-input" placeholder="Filter by library or license" aria-label="Filter licenses" value={query} onChange={(e) => setQuery(e.target.value)} />
      {credits === undefined && <p className="small ao-dim">Loading…</p>}
      {credits === null && <p className="small ao-dim">This build has no credits file. A release build generates it.</p>}
      {credits && !credits.rust && <p className="small ao-dim">This build lists the npm packages only: cargo-about was missing when it was made.</p>}
      <div className="licenses-list">
        {shown?.map((g) => (
          <details key={`${g.license} ${g.packages[0]}`} className="license-group">
            <summary>
              <b>{g.license}</b> <span className="small ao-dim">{g.packages.join(", ")}</span>
            </summary>
            {g.text ? <pre className="paste-preview">{g.text}</pre> : <p className="small ao-dim">No license file ships with this package.</p>}
          </details>
        ))}
      </div>
      <div className="srow">
        <span className="grow" />
        <button type="button" className="ao-btn primary" onClick={onClose}>
          Close
        </button>
      </div>
    </Sheet>
  );
}

/** Help › Privacy. */
export function PrivacySheet({ onClose }: { onClose(): void }) {
  return (
    <Sheet label="Privacy" className="privacy-sheet" onClose={onClose}>
      <h3 className="ao-pixel ao-dim">Privacy</h3>
      <ul className="privacy-points">
        {PRIVACY_POINTS.map((p) => (
          <li key={p}>{p}</li>
        ))}
      </ul>
      <div className="srow">
        <span className="grow" />
        <button type="button" className="ao-btn primary" onClick={onClose}>
          Close
        </button>
      </div>
    </Sheet>
  );
}

/** Help › Send Feedback and Report an Issue (D7): the log only with the box ticked. */
export function FeedbackSheet({ host, onError, onClose, route }: AppSheetProps & { route: FeedbackRoute }) {
  const [log, setLog] = useState(false);
  const go = () =>
    void host.feedbackOpen(route, log).then(onClose, (e: unknown) => onError(String(e)));
  const email = route === "email";
  return (
    <Sheet label={email ? "Send Feedback" : "Report an Issue"} className="feedback-sheet" onClose={onClose}>
      <h3 className="ao-pixel ao-dim">{email ? "Send feedback" : "Report an issue"}</h3>
      <p className="small">
        {email
          ? "Your mail app opens a draft to PANTOPIX RO with the app and macOS versions filled in. Nothing is sent until you send it."
          : "Your browser opens a new GitHub issue with the app and macOS versions filled in. Nothing is posted until you submit it."}
      </p>
      <label className="srow">
        <input type="checkbox" checked={log} onChange={(e) => setLog(e.target.checked)} />
        <span>Include app.log</span>
        <span className="small ao-dim grow">Finder shows the file for you to attach. It can hold file paths and pane titles.</span>
      </label>
      <div className="srow">
        <span className="grow" />
        <button type="button" className="ao-btn ghost" onClick={onClose}>
          Cancel
        </button>
        <button type="button" className="ao-btn primary" onClick={go}>
          {email ? "Open email" : "Open issue"}
        </button>
      </div>
    </Sheet>
  );
}

/** Help › Uninstall Terminescu Helpers (D9): asks, removes, then says how to finish. */
export function UninstallSheet({ host, onError, onClose }: AppSheetProps) {
  const [data, setData] = useState(false);
  const [done, setDone] = useState(false);
  const [busy, setBusy] = useState(false);
  if (done)
    return (
      <Sheet label="Helpers removed" className="uninstall-sheet" onClose={() => void host.appQuit()}>
        <h3 className="ao-pixel ao-dim">Helpers removed</h3>
        <p>Terminescu quits now. Drag Terminescu from Applications to the Trash to finish.</p>
        <div className="srow">
          <span className="grow" />
          <button type="button" className="ao-btn primary" onClick={() => void host.appQuit()}>
            Quit
          </button>
        </div>
      </Sheet>
    );
  return (
    <Sheet label="Uninstall Terminescu Helpers" className="uninstall-sheet" onClose={onClose}>
      <h3 className="ao-pixel ao-dim">Uninstall Terminescu helpers</h3>
      <p className="small">This removes:</p>
      <ul className="small">
        <li>the Claude Code hook, from Claude Code&apos;s settings; your other hooks stay</li>
        <li>the terminescu command and the shell daemon from Terminescu&apos;s support folder</li>
        <li>every open shell in Terminescu, which closes now</li>
      </ul>
      <label className="srow">
        <input type="checkbox" checked={data} onChange={(e) => setData(e.target.checked)} />
        <span>Also delete my Terminescu data</span>
        <span className="small ao-dim grow">saved variables, commands, prompts, layout and logs</span>
      </label>
      <div className="srow">
        <span className="grow" />
        <button type="button" className="ao-btn ghost" onClick={onClose}>
          Cancel
        </button>
        <button
          type="button"
          className="ao-btn danger"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void host.uninstallHelpers(data).then(
              () => setDone(true),
              (e: unknown) => {
                setBusy(false);
                onError(String(e));
              },
            );
          }}
        >
          Uninstall
        </button>
      </div>
    </Sheet>
  );
}

/** The first-run screen (D6): what lives outside the bundle, and the hook question. */
export function FirstRunSheet({ host, onError, onClose, onPrivacy }: AppSheetProps & { onPrivacy(): void }) {
  const [hook, setHook] = useState(true);
  const finish = (on: boolean) =>
    void host
      .settings()
      .then((s) => host.setSettings({ ...s, hook: on ? "on" : "off", firstRunDone: true }))
      .then(onClose, (e: unknown) => onError(String(e)));
  return (
    <Sheet label="Welcome to Terminescu" className="first-run-sheet" onClose={() => finish(false)}>
      <h2 className="ao-pixel">TERMINESCU</h2>
      <p>A terminal manager for working in Claude Code all day: side tabs per ticket, split panes, and a side tab that turns amber when Claude waits for you.</p>
      <p className="small">To do that, Terminescu keeps two helpers in its support folder, outside the app:</p>
      <ul className="small">
        <li>
          the <code>terminescu</code> command, which your shells and the hook run
        </li>
        <li>the shell daemon, which keeps your shells running across app restarts and updates</li>
      </ul>
      <label className="srow">
        <input type="checkbox" checked={hook} onChange={(e) => setHook(e.target.checked)} />
        <span>Add the Claude Code hook</span>
        <span className="small ao-dim grow">
          Claude Code then tells Terminescu when a session starts, waits or ends. Without it the terminals work, but Claude status and the transcript pane stay off. Settings › Shell changes this later.
        </span>
      </label>
      <div className="srow">
        <button type="button" className="link small" onClick={onPrivacy}>
          Privacy
        </button>
        <span className="small ao-dim grow">{NOT_AFFILIATED}</span>
        <button type="button" className="ao-btn ghost" onClick={() => finish(false)}>
          Not now
        </button>
        <button type="button" className="ao-btn primary" onClick={() => finish(hook)}>
          Continue
        </button>
      </div>
    </Sheet>
  );
}
