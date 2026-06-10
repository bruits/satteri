/**
 * Shared machinery for walk-path child stubs (see `hast/child-stub.ts` and
 * `mdast/child-stub.ts`): lightweight stand-ins that carry arena id + type
 * eagerly and defer the full-arena snapshot until a plugin reads a real field.
 * Passthrough children (the common replaceNode shape) thus compile to one-word
 * refs without ever serializing the arena.
 */

/** The slice of `LazyChildResolver` a stub getter needs. */
interface StubResolver {
  materializeOne(id: number): object;
}

/** Stub host shape: `_id`/`_resolver` are enumerable plain fields for
 *  construction speed; spread/identity rules are enforced by the visitors' `nid()`. */
interface StubHost {
  _resolver: StubResolver;
  _id: number;
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
        real = this._resolver.materializeOne(this._id) as Record<string, unknown>;
        REAL_NODES.set(this, real);
      }
      const value = real[key];
      Object.defineProperty(this, key, {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
      });
      return value;
    };
    FIELD_GETTERS.set(key, getter);
  }
  return getter;
}

/**
 * Descriptor template for one node type's stub fields. Own enumerable getters
 * so a spread copy carries correct values. `position`/`data` ride on every
 * stub; they may yield `undefined` where a materialized node omits the key —
 * accepted drift, invisible to `toEqual`.
 */
export function stubDescriptors(fields: readonly string[]): PropertyDescriptorMap {
  const map: PropertyDescriptorMap = {};
  for (const key of [...fields, "position", "data"]) {
    map[key] = { get: fieldGetter(key), enumerable: true, configurable: true };
  }
  return map;
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
