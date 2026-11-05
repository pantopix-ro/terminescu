/**
 * A variable name for extracted text (D13): `pr_api` for a GitHub PR URL in the `api` repo,
 * `branch` for a branch name, `url` for another link, else `text`.
 */
export function suggestName(text: string): string {
  const t = text.trim();
  const pr = /^https:\/\/github\.com\/[^/\s]+\/([^/\s]+)\/pull\/\d+/.exec(t);
  if (pr) return `pr_${pr[1]!.toLowerCase().replace(/[^a-z0-9]/g, "_")}`;
  if (/^https?:\/\/\S+$/.test(t)) return "url";
  if (/^[A-Za-z0-9._-]+(\/[A-Za-z0-9._-]+)+$/.test(t)) return "branch";
  return "text";
}
