// The plugin seam (add-plugin-architecture D11). The core crates and the ui, host and
// plugin-api packages never reach a plugin, through a dependency or an import. Plugins are
// registered in two files only: apps/desktop/src-tauri/src/plugins.rs and
// apps/desktop/src/plugins.tsx. Exits 1 and lists every breach.
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), ".."));
const breaches = [];

/** A plugin, as opposed to the API every plugin builds on. */
const isPluginCrate = (name) => name.startsWith("terminescu-plugin-") && name !== "terminescu-plugin-api";
const isPluginPackage = (name) => name.startsWith("@terminescu/plugin-") && name !== "@terminescu/plugin-api";

// ---- Rust: the dependency graph from cargo metadata, every kind of dependency.

const GUARDED_CRATES = ["terminescu-core", "terminescu-protocol", "terminescu-ptyd", "terminescu-cli"];

const meta = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 << 20,
  }),
);
const nameOf = new Map(meta.packages.map((p) => [p.id, p.name]));
const deps = new Map(meta.resolve.nodes.map((n) => [n.id, n.deps.map((d) => d.pkg)]));
const members = new Set(meta.workspace_members);
for (const crate of GUARDED_CRATES) {
  if (![...members].some((id) => nameOf.get(id) === crate)) breaches.push(`crate ${crate} is not in the workspace`);
}

for (const id of members) {
  const crate = nameOf.get(id);
  if (!GUARDED_CRATES.includes(crate)) continue;
  // Breadth first, so the reported path is the shortest one.
  const via = new Map([[id, null]]);
  const queue = [id];
  while (queue.length) {
    const at = queue.shift();
    for (const next of deps.get(at) ?? []) {
      if (via.has(next)) continue;
      via.set(next, at);
      if (isPluginCrate(nameOf.get(next))) {
        const path = [];
        for (let p = next; p; p = via.get(p)) path.unshift(nameOf.get(p));
        breaches.push(`crate ${crate} reaches ${nameOf.get(next)}: ${path.join(" -> ")}`);
      } else {
        queue.push(next);
      }
    }
  }
}

// Rust registration: only plugins.rs names a plugin crate.
const RUST_REGISTRY = join("apps", "desktop", "src-tauri", "src", "plugins.rs");
const rustPlugin = /\bterminescu_plugin_(?!api\b)\w+/;
for (const file of walk(join(root, "apps", "desktop", "src-tauri", "src"), [".rs"])) {
  const rel = relative(root, file);
  if (rel === RUST_REGISTRY) continue;
  lines(file).forEach((line, i) => {
    if (rustPlugin.test(line)) breaches.push(`${rel}:${i + 1} names a plugin crate outside ${RUST_REGISTRY}`);
  });
}

// ---- TypeScript: package.json dependencies and the import specifiers in the sources.

const GUARDED_PACKAGES = ["ui", "host", "plugin-api"];
const SOURCE = [".ts", ".tsx", ".mts", ".js", ".mjs"];
const IMPORT = /(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s+|\brequire\s*\(\s*)["']([^"']+)["']/g;
const pluginDirs = readdirSync(join(root, "packages"))
  .filter((d) => d.startsWith("plugin-") && d !== "plugin-api")
  .map((d) => join(root, "packages", d));

for (const dir of GUARDED_PACKAGES) {
  const base = join(root, "packages", dir);
  const pkg = JSON.parse(readFileSync(join(base, "package.json"), "utf8"));
  for (const field of ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"]) {
    for (const name of Object.keys(pkg[field] ?? {})) {
      if (isPluginPackage(name)) breaches.push(`packages/${dir}/package.json lists ${name} in ${field}`);
    }
  }
  for (const file of walk(base, SOURCE)) {
    importsOf(file).forEach(({ spec, line }) => {
      const rel = `${relative(root, file)}:${line}`;
      if (isPluginPackage(packageOf(spec))) breaches.push(`${rel} imports ${spec}`);
      else if (spec.startsWith(".")) {
        const target = resolve(dirname(file), spec);
        if (pluginDirs.some((p) => target === p || target.startsWith(p + sep)))
          breaches.push(`${rel} imports ${spec}, inside a plugin package`);
      }
    });
  }
}

// UI registration: in the app, only plugins.tsx imports a plugin package.
const UI_REGISTRY = join("apps", "desktop", "src", "plugins.tsx");
for (const file of walk(join(root, "apps", "desktop", "src"), SOURCE)) {
  const rel = relative(root, file);
  if (rel === UI_REGISTRY) continue;
  importsOf(file).forEach(({ spec, line }) => {
    if (isPluginPackage(packageOf(spec))) breaches.push(`${rel}:${line} imports ${spec} outside ${UI_REGISTRY}`);
  });
}

if (breaches.length) {
  console.error(`check-plugins: ${breaches.length} breach(es) of the plugin seam:`);
  for (const b of breaches) console.error(`  ${b}`);
  process.exit(1);
}
console.log("check-plugins: the plugin seam holds");

// ---- helpers

function* walk(dir, exts) {
  for (const name of readdirSync(dir)) {
    if (name === "node_modules" || name === "dist" || name === "target" || name.startsWith(".")) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path, exts);
    else if (exts.some((e) => name.endsWith(e)) && !name.endsWith(".d.ts")) yield path;
  }
}

function lines(file) {
  return readFileSync(file, "utf8").split("\n");
}

/** Import specifiers with their 1-based line; type-only imports count too. */
function importsOf(file) {
  const out = [];
  lines(file).forEach((text, i) => {
    for (const m of text.matchAll(IMPORT)) out.push({ spec: m[1], line: i + 1 });
  });
  return out;
}

/** `@scope/name/sub` -> `@scope/name`, `name/sub` -> `name`. */
function packageOf(spec) {
  const parts = spec.split("/");
  return spec.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0];
}
