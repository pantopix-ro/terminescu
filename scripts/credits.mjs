// Writes apps/desktop/public/licenses.json, the open-source credits Help › Licenses shows
// (add-public-release D8): every Rust crate and npm package the app ships, grouped by license
// text. Rust comes from `cargo about` (about.toml), npm from `pnpm licenses`. The file is
// generated, not committed: license notices carry their authors' names and addresses.
// `--strict` or CREDITS_STRICT=1 (the release job) fails when cargo-about is missing.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const strict = process.argv.includes("--strict") || process.env.CREDITS_STRICT === "1";
const ours = (name) => name.startsWith("terminescu") || name.startsWith("@terminescu/");

/** license text -> { license, packages: Set } */
const groups = new Map();
function add(license, text, pkg) {
  const key = text.trim() || license;
  const g = groups.get(key) ?? { license, text: text.trim(), packages: new Set() };
  g.packages.add(pkg);
  groups.set(key, g);
}

let rust = true;
try {
  const out = execFileSync("cargo", ["about", "generate", "--workspace", "--format", "json"], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "ignore"],
  });
  for (const l of JSON.parse(out).licenses) {
    for (const u of l.used_by) {
      const c = u.crate;
      if (!ours(c.name)) add(l.id, l.text, `${c.name} ${c.version}`);
    }
  }
} catch (e) {
  if (strict) throw new Error(`cargo about failed; install it with \`cargo install --locked cargo-about@0.9.2 --features cli\`: ${e.message}`);
  rust = false;
  console.warn("credits: cargo-about is missing, so the Rust crates are left out of this build's credits");
}

const pnpm = JSON.parse(
  execFileSync(
    "pnpm",
    ["licenses", "list", "--json", "--prod", "--filter", "@terminescu/desktop", "--filter", "@terminescu/ui", "--filter", "@terminescu/host"],
    { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  ),
);
const LICENSE_FILE = /^(licen[cs]e|copying|notice)(\.|-|$)/i;
for (const [license, pkgs] of Object.entries(pnpm)) {
  for (const p of pkgs) {
    if (ours(p.name)) continue;
    for (const [i, version] of p.versions.entries()) {
      const dir = p.paths[i] ?? p.paths[0];
      const files = existsSync(dir) ? readdirSync(dir).filter((f) => LICENSE_FILE.test(f)).sort() : [];
      const text = files.map((f) => readFileSync(join(dir, f), "utf8")).join("\n\n");
      add(license, text, `${p.name} ${version}`);
    }
  }
}

const list = [...groups.values()]
  .map((g) => ({ license: g.license, packages: [...g.packages].sort(), text: g.text }))
  .sort((a, b) => a.license.localeCompare(b.license) || a.packages[0].localeCompare(b.packages[0]));
const out = join(root, "apps/desktop/public/licenses.json");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify({ rust, groups: list }) + "\n");
console.log(`credits: ${list.reduce((n, g) => n + g.packages.length, 0)} packages in ${list.length} license texts${rust ? "" : " (npm only)"}`);
