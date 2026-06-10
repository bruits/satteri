import { stubDescriptors } from "../child-stub.js";
import type { LazyChildResolver } from "../lazy-child-resolver.js";
import type { HastNode } from "../types.js";
import type { HastReader } from "./hast-reader.js";
import { NAME_TO_TYPE, TYPE_NAMES } from "./generated/node-types.js";

type HastResolver = LazyChildResolver<HastReader, HastNode>;

const N = NAME_TO_TYPE;

/** Per-type stub fields, mirroring `materializeHastNode`'s switch. */
const HAST_STUB_FIELDS: Readonly<Record<number, readonly string[]>> = {
  [N.root!]: ["children"],
  [N.element!]: ["tagName", "properties", "children"],
  [N.text!]: ["value"],
  [N.comment!]: ["value"],
  [N.doctype!]: [],
  [N.raw!]: ["value"],
  [N.mdxJsxFlowElement!]: ["name", "attributes", "children"],
  [N.mdxJsxTextElement!]: ["name", "attributes", "children"],
  [N.mdxFlowExpression!]: ["value"],
  [N.mdxTextExpression!]: ["value"],
  [N.mdxjsEsm!]: ["value"],
};

const HAST_STUB_DESCRIPTORS: ReadonlyMap<number, PropertyDescriptorMap> = new Map(
  Object.entries(HAST_STUB_FIELDS).map(([tag, fields]) => [Number(tag), stubDescriptors(fields)]),
);

/** Unknown node types still expose the prelude-backed lazy fields. */
const FALLBACK_DESCRIPTORS = stubDescriptors([]);

/**
 * Walk-path child stub: arena id + `type` eagerly, every other field a lazy
 * forward to the materialized node (first read snapshots the arena via
 * `materializeOne`, which enforces the pass seal). `nid()` recognizes genuine
 * stubs by `instanceof` and emits one-word refs; spread copies are not
 * `instanceof` and read as new content.
 */
export class HastChildStub {
  _resolver: HastResolver;
  _id: number;
  type: string;

  constructor(resolver: HastResolver, id: number, nodeType: number) {
    this._resolver = resolver;
    this._id = id;
    this.type = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;
    Object.defineProperties(this, HAST_STUB_DESCRIPTORS.get(nodeType) ?? FALLBACK_DESCRIPTORS);
  }
}
