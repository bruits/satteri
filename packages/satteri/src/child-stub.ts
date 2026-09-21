// Child stubs let unchanged children pass through as arena references without serializing the tree.

interface StubHost {
  _materialize(): object;
}

const REAL_NODES = new WeakMap<object, Record<string, unknown>>();

// Share getters across stubs to avoid allocating a closure for every field of every node.
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
      // Stub fields must remain as immutable as the frozen nodes they expose.
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

// Individual defineProperty calls avoid the descriptor-map validation cost of defineProperties.
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

// Dense node tags allow array indexing on the per-child hot path.
export function flatByTag<T>(table: Readonly<Record<number, T>>): readonly (T | undefined)[] {
  const flat: (T | undefined)[] = [];
  for (const tag of Object.keys(table)) {
    const nodeType = Number(tag);
    flat[nodeType] = table[nodeType];
  }
  return flat;
}
