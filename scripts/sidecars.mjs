// Builds terminescu-ptyd and terminescu and copies them where Tauri's externalBin expects them:
// apps/desktop/src-tauri/binaries/<name>-<target-triple> (D19). Tauri sets TAURI_ENV_DEBUG for
// `tauri build --debug`; any other run builds release binaries.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, chmodSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const debug = process.env.TAURI_ENV_DEBUG === "true" || process.argv.includes("--debug");
const triple =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  /host: (\S+)/.exec(execFileSync("rustc", ["-vV"], { encoding: "utf8" }))[1];

const args = ["build", "-p", "terminescu-ptyd", "-p", "terminescu-cli", "--target", triple];
if (!debug) args.push("--release");
execFileSync("cargo", args, { cwd: root, stdio: "inherit" });

const out = join(root, "apps/desktop/src-tauri/binaries");
mkdirSync(out, { recursive: true });
for (const bin of ["terminescu-ptyd", "terminescu"]) {
  const from = join(root, "target", triple, debug ? "debug" : "release", bin);
  const to = join(out, `${bin}-${triple}`);
  copyFileSync(from, to);
  chmodSync(to, 0o755);
  console.log(`sidecar ${to}`);
}
