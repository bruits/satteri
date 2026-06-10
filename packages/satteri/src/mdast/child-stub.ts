import { flatByTag, stubDescriptors } from "../child-stub.js";
import type { LazyChildResolver } from "../lazy-child-resolver.js";
import type { MdastNode } from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import { MDAST_LAYOUT_KEYS } from "./generated/layout.js";
import { NAME_TO_TYPE, TYPE_NAMES } from "./generated/node-types.js";
import { LEAF_TYPES } from "./mdast-materializer.js";

type MdastResolver = LazyChildResolver<MdastReader, MdastNode>;

const N = NAME_TO_TYPE;

/** Per-type stub fields for the types `addTypeProperties` hand-writes; the
 *  fixed-layout types come from the generated `MDAST_LAYOUT_KEYS`. */
const HAND_WRITTEN_FIELDS: Readonly<Record<number, readonly string[]>> = {
  [N.list!]: ["ordered", "start", "spread"],
  [N.listItem!]: ["spread", "checked"],
  [N.table!]: ["align"],
  [N.containerDirective!]: ["name", "attributes"],
  [N.leafDirective!]: ["name", "attributes"],
  [N.textDirective!]: ["name", "attributes"],
  [N.mdxJsxFlowElement!]: ["name", "attributes"],
  [N.mdxJsxTextElement!]: ["name", "attributes"],
};

const TYPE_NAME_BY_TAG = flatByTag(TYPE_NAMES);

const MDAST_STUB_DESCRIPTORS: (PropertyDescriptorMap | undefined)[] = [];
for (const tag of Object.keys(TYPE_NAMES)) {
  const nodeType = Number(tag);
  const fields = [...(MDAST_LAYOUT_KEYS[nodeType] ?? HAND_WRITTEN_FIELDS[nodeType] ?? [])];
  if (!LEAF_TYPES.has(nodeType)) fields.push("children");
  MDAST_STUB_DESCRIPTORS[nodeType] = stubDescriptors(fields);
}

/** Unknown node types still expose the prelude-backed lazy fields. */
const FALLBACK_DESCRIPTORS = stubDescriptors([]);

/**
 * Walk-path child stub: arena id + `type` eagerly, every other field a lazy
 * forward to the materialized node (first read snapshots the arena via
 * `materializeOne`, which enforces the handle epoch — the pass seal is checked
 * where the stubs are built). Spread/identity rules are enforced by `nid()`
 * (authoritative doc in hast-visitor.ts).
 */
export class MdastChildStub {
  _resolver: MdastResolver;
  _id: number;
  type: string;

  constructor(resolver: MdastResolver, id: number, nodeType: number) {
    this._resolver = resolver;
    this._id = id;
    this.type = TYPE_NAME_BY_TAG[nodeType] ?? `unknown(${nodeType})`;
    Object.defineProperties(this, MDAST_STUB_DESCRIPTORS[nodeType] ?? FALLBACK_DESCRIPTORS);
  }
}
