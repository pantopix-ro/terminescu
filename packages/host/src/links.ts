// The mock host's copy of the core's link rules (refine-context-links D1, D2, D6): the same
// WHATWG parser, so e2e sees what the app shows. `crates/core/src/links.rs` is the source.

export type LinkType = "pr" | "artifact" | "web";

const TRACKING = new Set(["fbclid", "gclid", "dclid", "msclkid", "mc_cid", "mc_eid", "igshid", "_hsenc", "_hsmi"]);
const ID = /^[A-Za-z0-9_-]+$/;
const PART = /^[A-Za-z0-9._-]+$/;

/** The cleaned form of an http(s) link, or `null` when `raw` is not one. */
export function cleanLink(raw: string): string | null {
  const text = raw.trim();
  if (!text || /\s/.test(text)) return null;
  let url: URL;
  try {
    url = new URL(text);
  } catch {
    return null;
  }
  if ((url.protocol !== "http:" && url.protocol !== "https:") || !url.hostname) return null;
  url.username = "";
  url.password = "";
  const all = [...url.searchParams];
  const kept = all.filter(([k]) => {
    const key = k.toLowerCase();
    return !key.startsWith("utm_") && !TRACKING.has(key);
  });
  if (kept.length < all.length) url.search = kept.length ? new URLSearchParams(kept).toString() : "";
  const seg = url.pathname.split("/").filter(Boolean);
  const host = url.hostname;
  if ((host === "github.com" || host === "www.github.com") && seg[2] === "pull") {
    const [owner, repo, , n] = seg;
    if (owner && repo && n && PART.test(owner) && PART.test(repo) && /^\d+$/.test(n)) {
      return `https://github.com/${owner.toLowerCase()}/${repo.toLowerCase()}/pull/${n}`;
    }
  }
  if (host === "claude.ai") {
    if (seg[0] === "artifact" && seg[1] && ID.test(seg[1])) return `https://claude.ai/artifact/${seg[1]}`;
    if (seg[0] === "public" && seg[1] === "artifacts" && seg[2] && ID.test(seg[2])) return `https://claude.ai/public/artifacts/${seg[2]}`;
    if (seg[0] === "code" && seg[1] === "artifact" && seg[2] && ID.test(seg[2])) return `https://claude.ai/code/artifact/${seg[2]}`;
  }
  if (url.pathname.length > 1 && url.pathname.endsWith("/")) url.pathname = url.pathname.replace(/\/+$/, "");
  return url.href.replace(/#$/, "");
}

function prParts(url: string): [string, number] | null {
  const m = /^https:\/\/github\.com\/[^/]+\/([^/]+)\/pull\/(\d+)$/.exec(url);
  return m ? [m[1]!, Number(m[2])] : null;
}

export function linkType(url: string): LinkType {
  if (prParts(url)) return "pr";
  if (url.startsWith("https://claude.ai/") && url.includes("artifact")) return "artifact";
  return "web";
}

/** The label of a link without a name. */
export function linkLabel(url: string): string {
  const type = linkType(url);
  if (type === "pr") {
    const [repo, n] = prParts(url)!;
    return `PR #${n} · ${repo}`;
  }
  if (type === "artifact") return "Artifact";
  try {
    const u = new URL(url);
    const host = u.hostname.replace(/^www\./, "");
    const last = u.pathname.split("/").filter(Boolean).pop();
    return last ? `${host} · ${last}` : host;
  } catch {
    return url;
  }
}
