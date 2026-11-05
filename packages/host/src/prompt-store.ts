import type { NewPrompt, Prompt, PromptVersion } from "./protocol";
import { variablesOf } from "./protocol";

/** `Review the plan!` becomes `review-the-plan`; the core's `key_of`. */
export function keyOf(name: string): string {
  const key = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 48)
    .replace(/-+$/, "");
  return key || "prompt";
}

function cleanTags(tags: string[]): string[] {
  const out: string[] = [];
  for (const t of tags) {
    const tag = t.trim().replace(/^#+/, "").toLowerCase();
    if (tag && !out.includes(tag)) out.push(tag);
  }
  return out;
}

/** The library search of the core's `prompts::search`, for the mock host. */
export function matches(p: Prompt, query: string): boolean {
  const name = p.name.toLowerCase();
  const body = p.body.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => {
      const tag = w.startsWith("#") ? w.slice(1) : "";
      if (tag) return p.tags.includes(tag);
      return name.includes(w) || body.includes(w) || p.tags.some((t) => t.includes(w));
    });
}

/** An in-memory prompt library with the core's rules: versions only grow, keys stay unique. */
export class PromptStore {
  private prompts = new Map<string, Prompt>();
  private history = new Map<string, PromptVersion[]>();
  private next = 0;

  constructor(private readonly changed: () => void = () => {}) {}

  private get(id: string): Prompt {
    const p = this.prompts.get(id);
    if (!p) throw new Error(`no prompt ${id}`);
    return p;
  }

  private freeKey(key: string): string {
    const taken = (k: string) => [...this.prompts.values()].some((p) => p.key === k);
    if (!taken(key)) return key;
    for (let n = 2; ; n++) if (!taken(`${key}-${n}`)) return `${key}-${n}`;
  }

  private push(p: Prompt, body: string): Prompt {
    const now = Date.now();
    const version = p.version + 1;
    this.history.get(p.id)!.unshift({ version, body, createdAt: now });
    Object.assign(p, { version, body, variables: variablesOf(body), updatedAt: now });
    this.changed();
    return structuredClone(p);
  }

  search(query: string, archived = false): Prompt[] {
    return [...this.prompts.values()]
      .filter((p) => p.archived === archived && matches(p, query))
      .sort((a, b) => (b.lastUsedAt ?? 0) - (a.lastUsedAt ?? 0) || a.name.toLowerCase().localeCompare(b.name.toLowerCase()))
      .map((p) => structuredClone(p));
  }

  create(n: NewPrompt): Prompt {
    const name = n.name.trim();
    if (!name) throw new Error("a prompt needs a name");
    if (!n.body.trim()) throw new Error("the prompt body is empty");
    if (n.key && [...this.prompts.values()].some((p) => p.key === n.key)) throw new Error(`a prompt with the key ${n.key} exists`);
    const now = Date.now();
    const p: Prompt = {
      id: `prompt-${++this.next}`,
      key: n.key || this.freeKey(keyOf(name)),
      name,
      tags: cleanTags(n.tags),
      archived: false,
      version: 1,
      body: n.body,
      variables: variablesOf(n.body),
      lastUsedAt: null,
      useCount: 0,
      createdAt: now,
      updatedAt: now,
    };
    this.prompts.set(p.id, p);
    this.history.set(p.id, [{ version: 1, body: n.body, createdAt: now }]);
    this.changed();
    return structuredClone(p);
  }

  save(id: string, body: string): Prompt {
    if (!body.trim()) throw new Error("the prompt body is empty");
    const p = this.get(id);
    return p.body === body ? structuredClone(p) : this.push(p, body);
  }

  update(id: string, name: string, tags: string[]): Prompt {
    if (!name.trim()) throw new Error("a prompt needs a name");
    const p = this.get(id);
    Object.assign(p, { name: name.trim(), tags: cleanTags(tags), updatedAt: Date.now() });
    this.changed();
    return structuredClone(p);
  }

  versions(id: string): PromptVersion[] {
    this.get(id);
    return structuredClone(this.history.get(id)!);
  }

  restore(id: string, version: number): Prompt {
    const old = this.history.get(id)?.find((v) => v.version === version);
    if (!old) throw new Error(`prompt ${id} has no version ${version}`);
    return this.push(this.get(id), old.body);
  }

  duplicate(id: string): Prompt {
    const p = this.get(id);
    return this.create({ name: `${p.name} copy`, tags: p.tags, body: p.body });
  }

  archive(id: string, archived: boolean): Prompt {
    const p = this.get(id);
    p.archived = archived;
    p.updatedAt = Date.now();
    this.changed();
    return structuredClone(p);
  }

  markUsed(id: string): void {
    const p = this.get(id);
    p.lastUsedAt = Date.now();
    p.useCount++;
    this.changed();
  }
}
