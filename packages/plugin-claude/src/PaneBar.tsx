// The Claude bars above a terminal pane (harden-plugin-host D7): the hook is off while Claude
// runs there, or the pane offers the session that ended with the last restart.

import type { PaneBarProps } from "@terminescu/plugin-api";

export function ClaudePaneBar({ program, resume, app }: PaneBarProps) {
  return (
    <>
      {!app.hookOn && program === "claude" && (
        <div className="ao-bar-wait resume-bar hook-off-bar" role="status">
          <span className="w">Claude hook is off</span>
          <span className="grow small ao-dim">no Claude status or transcript for this session</span>
          <button type="button" className="ao-btn primary" onClick={() => app.setHook(true)}>
            Turn on
          </button>
        </div>
      )}
      {resume && (
        <div className="ao-bar-wait resume-bar" role="status">
          <span className="w">Claude session {resume}</span>
          <span className="grow small ao-dim">ended with the last restart</span>
          <button type="button" className="ao-btn primary" onClick={app.resume}>
            Resume
          </button>
          <button type="button" className="ao-btn ghost" onClick={app.dismiss}>
            Dismiss
          </button>
        </div>
      )}
    </>
  );
}
