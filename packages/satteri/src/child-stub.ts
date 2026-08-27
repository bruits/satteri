/**
 * Shared machinery for walk-path child stubs (see `hast/child-stub.ts` and
 * `mdast/child-stub.ts`): lightweight stand-ins that carry arena id + type
 * eagerly and defer the full-arena snapshot until a plugin reads a real field.
 * Passthrough children (the common replaceNode shape) thus compile to one-word
 * refs without ever serializing the arena.
 */

/** Stub host shape: the arena id and resolver stay private so a
 *  `structuredClone` of a stub carries node data only. */
interface StubHost {
  _materialize(): object;
}

/** Stub → materialized arena node, filled on the first real-field read. */
const REAL_NODES = new WeakMap<object, Record<string, unknown>>();

/** One shared memoizing getter per field name, so stubs install shared
 *  descriptor templates instead of allocating per-field closures per node. */
const FIELD_GETTERS = new Map<string, (this: StubHost) => unknown>();

function fieldGetter(key: string): (this: StubHost) => unknown {
  let getter = FIELD_GETTERS.get(key);
  if (getter === undefined) {
    getter = function (this: StubHost) {
      let real = REAL_NODES.get(this);
      if (real === undefined) {
        real = this._materialize() as Record<string, unknown>;
        REAL_NODES.set(this, real);
      }
      const value = real[key];
      // Mirrors the deep-frozen node it forwards to; nothing redefines a stub field after this.
      Object.defineProperty(this, key, {
        value,
        writable: false,
        enumerable: true,
        configurable: false,
      });
      return value;
    };
    FIELD_GETTERS.set(key, getter);
  }
  return getter;
}

export type StubDescriptorEntry = readonly [string, PropertyDescriptor];

/**
 * Descriptor entries for one node type's stub fields. Own enumerable getters
 * so a spread copy carries correct values. `position`/`data` ride on every
 * stub; they may yield `undefined` where a materialized node omits the key —
 * accepted drift, invisible to `toEqual`. Installed one `defineProperty` at a
 * time: `Object.defineProperties` re-validates the whole map per call and
 * costs nearly double.
 */
export function stubDescriptors(fields: readonly string[]): readonly StubDescriptorEntry[] {
  const entries: StubDescriptorEntry[] = [];
  for (const key of [...fields, "position", "data"]) {
    entries.push([key, { get: fieldGetter(key), enumerable: true, configurable: true }]);
  }
  return entries;
}

export function installStubDescriptors(
  host: object,
  entries: readonly StubDescriptorEntry[],
): void {
  for (let i = 0; i < entries.length; i++) {
    const e = entries[i];
    if (e === undefined) continue;
    Object.defineProperty(host, e[0], e[1]);
  }
}

/** Node tags are dense small ints, so the per-child stub loop indexes flat
 *  arrays instead of paying Map/dictionary lookups. */
export function flatByTag<T>(table: Readonly<Record<number, T>>): readonly (T | undefined)[] {
  const flat: (T | undefined)[] = [];
  for (const tag of Object.keys(table)) {
    const nodeType = Number(tag);
    flat[nodeType] = table[nodeType];
  }
  return flat;
}
