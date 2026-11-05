/**
 * A variable name for extracted text (D13): `pr_api` for a GitHub PR URL in the `api` repo,
 * `plan` for a claude.ai artifact URL, `branch` for a branch name, `url` for another link, else
 * `text`. A name in `taken` gets the next free `_2`, `_3`; a second artifact becomes
 * `artifact_2`.
 */
export function suggestName(text: string, taken: Iterable<string> = []): string {
  const used = new Set(taken);
  const t = text.trim();
  const free = (base: string) => {
    if (!used.has(base)) return base;
    for (let n = 2; ; n++) if (!used.has(`${base}_${n}`)) return `${base}_${n}`;
  };
  const pr = /^https:\/\/github\.com\/[^/\s]+\/([^/\s]+)\/pull\/\d+/.exec(t);
  if (pr) return free(`pr_${pr[1]!.toLowerCase().replace(/[^a-z0-9]/g, "_")}`);
  if (/^https:\/\/claude\.ai\/(artifact|public\/artifacts)\/\S+$/.test(t)) {
    return used.has("plan") ? free("artifact") : "plan";
  }
  if (/^https?:\/\/\S+$/.test(t)) return free("url");
  if (/^[A-Za-z0-9._-]+(\/[A-Za-z0-9._-]+)+$/.test(t)) return free("branch");
  return free("text");
}

/** `30s`, `45m`, `1h`, `2d` in seconds; undefined for anything else. */
export function parseTtl(text: string): number | undefined {
  const m = /^\s*(\d+)\s*([smhd])\s*$/.exec(text);
  if (!m) return undefined;
  const n = Number(m[1]);
  const secs = n * { s: 1, m: 60, h: 3600, d: 86_400 }[m[2] as "s" | "m" | "h" | "d"];
  return secs > 0 ? secs : undefined;
}

/** A TTL in seconds as `parseTtl` reads it back: `2d`, `1h`, `45m` or `30s`. */
export function ttlText(secs: number): string {
  for (const [unit, n] of [["d", 86_400], ["h", 3600], ["m", 60]] as const) if (secs % n === 0) return `${secs / n}${unit}`;
  return `${secs}s`;
}

/**
 * The extraction rules of v1 (D13), read-only in Settings: what each one recognises and the
 * name `suggestName` gives it.
 */
export const EXTRACTION_RULES: readonly { what: string; example: string; name: string }[] = [
  { what: "GitHub PR URL", example: "https://github.com/acme/api/pull/918", name: "pr_api, after the repo" },
  { what: "claude.ai artifact URL", example: "https://claude.ai/artifact/…", name: "plan, then artifact_2" },
  { what: "Branch name", example: "feature/audit-export", name: "branch" },
  { what: "Other link", example: "https://example.com/runbook", name: "url" },
];

/** `52m left`, `3h left`, `20s left` for an expiry at Unix ms `at`. */
/**
 * What inserting a variable types: a secret goes in as `$NAME`, which the shell and Claude's
 * Bash tool expand from the environment (D35); others go in as their value, minus a trailing
 * line break so an insert never runs a command.
 */
export function insertText(v: { name: string; mode: string; value: string | null }): string {
  if (v.mode === "secret") return `$${v.name}`;
  return (v.value ?? "").replace(/[\r\n]+$/, "");
}

export function timeLeft(at: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((at - now) / 1000));
  if (s < 60) return `${s}s left`;
  if (s < 3600) return `${Math.floor(s / 60)}m left`;
  if (s < 86_400) return `${Math.floor(s / 3600)}h left`;
  return `${Math.floor(s / 86_400)}d left`;
}

/** `just now`, `5m ago`, `3h ago`, `2d ago`. */
export function timeAgo(at: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - at) / 1000));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86_400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86_400)}d ago`;
}
