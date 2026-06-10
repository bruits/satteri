import { stubDescriptors } from "../child-stub.js";
import type { LazyChildResolver } from "../lazy-child-resolver.js";
import type { MdastNode } from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import { MDAST_LAYOUT_KEYS } from "./generated/layout.js";
import { TYPE_NAMES } from "./generated/node-types.js";
import { LEAF_TYPES } from "./mdast-materializer.js";

type MdastResolver = LazyChildResolver<MdastReader, MdastNode>;

/** Per-type stub fields for the types `addTypeProperties` hand-writes; the
 *  fixed-layout types come from the generated `MDAST_LAYOUT_KEYS`. */
const HAND_WRITTEN_FIELDS: Readonly<Record<number, readonly string[]>> = {
  5: ["ordered", "start", "spread"],
  6: ["spread", "checked"],
  21: ["align"],
  30: ["name", "attributes"],
  31: ["name", "attributes"],
  32: ["name", "attributes"],
  100: ["name", "attributes"],
  101: ["name", "attributes"],
};

const MDAST_STUB_DESCRIPTORS: ReadonlyMap<number, PropertyDescriptorMap> = new Map(
  Object.keys(TYPE_NAMES).map((tag) => {
    const nodeType = Number(tag);
    const fields = [...(MDAST_LAYOUT_KEYS[nodeType] ?? HAND_WRITTEN_FIELDS[nodeType] ?? [])];
    if (!LEAF_TYPES.has(nodeType)) fields.push("children");
    return [nodeType, stubDescriptors(fields)];
  }),
);

/** Unknown node types still expose the prelude-backed lazy fields. */
const FALLBACK_DESCRIPTORS = stubDescriptors([]);

/**
 * Walk-path child stub: arena id + `type` eagerly, every other field a lazy
 * forward to the materialized node (first read snapshots the arena via
 * `materializeOne`, which enforces the pass seal). `nid()` recognizes genuine
 * stubs by `instanceof` and emits one-word refs; spread copies are not
 * `instanceof`, carry no `_nodeId`, and read as new content.
 */
export class MdastChildStub {
  _resolver: MdastResolver;
  _id: number;
  type: string;

  constructor(resolver: MdastResolver, id: number, nodeType: number) {
    this._resolver = resolver;
    this._id = id;
    this.type = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;
    Object.defineProperties(this, MDAST_STUB_DESCRIPTORS.get(nodeType) ?? FALLBACK_DESCRIPTORS);
  }
}
