/** A link under the pointer in a terminal (task 8.49): its kind picks the right-click menu. */
export type LinkKind = "url" | "path" | "email";
export interface TerminalLink {
  kind: LinkKind;
  text: string;
}

// A plain address; `mailto:` before it stays out of the link.
const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+/g;

/** Email addresses in one line of terminal text, with their string offsets (end exclusive). */
export function findEmails(line: string): { text: string; start: number; end: number }[] {
  const out: { text: string; start: number; end: number }[] = [];
  for (const m of line.matchAll(EMAIL)) {
    const before = line[m.index - 1];
    // Part of a URL (`https://user@host`) or a path is the other providers' link.
    if (before === "/" || (before === ":" && !line.slice(0, m.index).endsWith("mailto:"))) continue;
    let text = m[0].replace(/[.-]+$/, "");
    if (text.startsWith(".") || !text.includes("@")) continue;
    out.push({ text, start: m.index, end: m.index + text.length });
  }
  return out;
}

/** The right-click menu's entries for a link, as ids and labels; the app runs them. */
export function linkActions(kind: LinkKind): { id: string; label: string }[] {
  switch (kind) {
    case "url":
      return [
        { id: "browser", label: "Open in Browser" },
        { id: "copy", label: "Copy Link" },
      ];
    case "path":
      return [
        { id: "text", label: "Open in Text Tab" },
        { id: "reveal", label: "Reveal in Finder" },
        { id: "default", label: "Open with Default App" },
        { id: "copy", label: "Copy Path" },
      ];
    case "email":
      return [
        { id: "mail", label: "Compose in Mail" },
        { id: "copy", label: "Copy Address" },
      ];
  }
}
