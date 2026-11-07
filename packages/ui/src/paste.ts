/** A paste longer than this asks first (task 8.42). */
export const PASTE_LIMIT = 5_000;

/**
 * Why a paste should ask before it reaches the shell, or undefined when it can go: a long text,
 * a line that starts with `sudo`, or a pipe into a shell such as `curl … | sh`.
 */
export function pasteRisk(text: string): string | undefined {
  if (text.length > PASTE_LIMIT) return `It is ${text.length.toLocaleString("en-US")} characters long.`;
  if (/^\s*sudo\b/m.test(text)) return "It runs a command with sudo.";
  if (/\|\s*(sudo\s+)?(\S*\/)?(ba|z|da|k|fi)?sh\b/.test(text)) return "It pipes text into a shell.";
  return undefined;
}
