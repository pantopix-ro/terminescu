import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { isVariableName, SHELL_CONTROL_NAMES } from "../src/protocol";

// The TS side reads the Rust sources it mirrors, so a change on one side fails here.

const root = new URL("../../../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), "utf8");

describe("the Rust mirror", () => {
  it("every command ipc.ts invokes is registered in the app", () => {
    const ipc = read("packages/host/src/ipc.ts");
    const main = read("apps/desktop/src-tauri/src/main.rs");
    const list = main.slice(main.indexOf("generate_handler!["), main.indexOf("])", main.indexOf("generate_handler![")));
    const registered = new Set([...list.matchAll(/(?:\w+::)*(\w+)\s*,?/g)].map((m) => m[1]));
    const invoked = [...ipc.matchAll(/invoke(?:<[^>]*>)?\(\s*"(\w+)"/g)].map((m) => m[1]!);
    expect(invoked.length).toBeGreaterThan(50);
    expect(invoked.filter((name) => !registered.has(name))).toEqual([]);
  });

  it("the names the shell acts on match crates/protocol", () => {
    const lib = read("crates/protocol/src/lib.rs");
    const body = lib.slice(lib.indexOf("fn is_shell_control"), lib.indexOf("];", lib.indexOf("fn is_shell_control")));
    const rust = [...body.matchAll(/"(\w+)"/g)].map((m) => m[1]);
    expect(SHELL_CONTROL_NAMES).toEqual(rust);
    for (const name of ["PATH", "PROMPT_COMMAND", "DYLD_INSERT_LIBRARIES", "BASH_FUNC_x", "TERMINESCU_VARS"]) {
      expect(isVariableName(name), name).toBe(false);
    }
    expect(isVariableName("API_TOKEN")).toBe(true);
  });
});
