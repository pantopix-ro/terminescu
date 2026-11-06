// The mock's copy of the core's pane labels (task 8.30): `crates/core/src/labels.rs` for the
// label, `title_of` in `crates/ptyd/src/osc.rs` for the title, `NAMES` in state.rs for names.
import type { PaneLabel } from "./protocol";

/** The mock's home folder, and every pane's cwd until a test sets one. */
export const MOCK_HOME = "/Users/owner";

export const NAMES = [
  "Otter", "Heron", "Lynx", "Wren", "Ibis", "Fox", "Badger", "Marten", "Stoat", "Hare", "Bison",
  "Moose", "Elk", "Ibex", "Yak", "Llama", "Alpaca", "Tapir", "Okapi", "Panda", "Koala", "Lemur",
  "Gecko", "Iguana", "Newt", "Toad", "Seal", "Walrus", "Orca", "Dolphin", "Narwhal", "Beaver",
  "Mole", "Shrew", "Vole", "Hedgehog", "Puma", "Jaguar", "Ocelot", "Cheetah", "Zebra", "Gazelle",
  "Impala", "Camel", "Dingo", "Coyote", "Wolf", "Bear", "Raccoon", "Ferret", "Mink", "Sloth",
  "Wombat", "Quokka", "Meerkat", "Mongoose", "Gibbon", "Tiger", "Leopard", "Robin", "Finch",
  "Lark", "Swift", "Crane", "Stork", "Egret", "Kestrel", "Falcon", "Osprey", "Owl", "Raven",
  "Magpie", "Jay", "Puffin", "Gannet", "Tern", "Plover", "Kiwi", "Toucan", "Parrot", "Macaw",
  "Pelican", "Condor", "Sparrow", "Starling", "Swallow", "Oriole", "Hoopoe", "Grouse", "Quail",
  "Dove", "Curlew", "Snipe", "Avocet",
] as const;

const CLAUDE_DEFAULT = "Claude Code";
const MAX_PROGRAM = 40;
const MAX_TITLE = 200;
const WRAPPERS = ["sudo", "env", "exec", "time", "nohup", "command", "builtin", "nice", "caffeinate", "noglob"];
const TITLE_GLYPHS = /^[\s✳◐◑◒◓⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]+/u;

/** A raw OSC 0/2 title without control characters and its leading spinner; undefined when blank. */
export function titleOf(raw: string): string | undefined {
  // eslint-disable-next-line no-control-regex
  const clean = [...raw.replace(/[\u0000-\u001f\u007f-\u009f]/g, "").replace(TITLE_GLYPHS, "").trimEnd()].slice(0, MAX_TITLE).join("");
  return clean || undefined;
}

export function homePath(cwd: string, home: string): string {
  const h = home.replace(/\/+$/, "");
  if (!h) return cwd;
  if (cwd === h) return "~";
  return cwd.startsWith(`${h}/`) ? `~${cwd.slice(h.length)}` : cwd;
}

/** The program a command line runs, never its arguments. */
export function programOf(command: string): string | undefined {
  let wrapped = false;
  for (const word of command.split(/\s+/).filter(Boolean)) {
    if (word.startsWith("-") && wrapped) continue;
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(word)) continue;
    const name = word.slice(word.lastIndexOf("/") + 1);
    if (WRAPPERS.includes(name)) {
      wrapped = true;
      continue;
    }
    const kept = [...name].slice(0, MAX_PROGRAM).join("");
    return kept || undefined;
  }
  return undefined;
}

export function paneLabel(cwd: string | undefined, running: string | undefined, title: string | undefined, claude: boolean, home = MOCK_HOME): PaneLabel {
  const out: PaneLabel = {};
  if (cwd !== undefined) out.path = homePath(cwd, home);
  const program = running === undefined ? undefined : programOf(running);
  if (program) out.program = program;
  if (claude && title && title !== CLAUDE_DEFAULT) out.title = title;
  return out;
}
