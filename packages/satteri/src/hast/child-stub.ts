import {
  flatByTag,
  installStubDescriptors,
  stubDescriptors,
  type StubDescriptorEntry,
} from "../child-stub.js";
import type { LazyChildResolver } from "../lazy-child-resolver.js";
import type { HastNode } from "../types.js";
import type { HastReader } from "./hast-reader.js";
import { NAME_TO_TYPE, TYPE_NAMES } from "./generated/node-types.js";
import { HAST_CONTAINER_TYPES } from "./hast-materializer.js";

type HastResolver = LazyChildResolver<HastReader, HastNode>;

const N = NAME_TO_TYPE;

/** Per-type stub fields; must mirror `materializeHastNode`'s populate switch. */
const HAST_STUB_FIELDS: Readonly<Record<number, readonly string[]>> = {
  [N.root!]: [],
  [N.element!]: ["tagName", "properties"],
  [N.text!]: ["value"],
  [N.comment!]: ["value"],
  [N.doctype!]: [],
  [N.raw!]: ["value"],
  [N.mdxJsxFlowElement!]: ["name", "attributes"],
  [N.mdxJsxTextElement!]: ["name", "attributes"],
  [N.mdxFlowExpression!]: ["value"],
  [N.mdxTextExpression!]: ["value"],
  [N.mdxjsEsm!]: ["value"],
};

const TYPE_NAME_BY_TAG = flatByTag(TYPE_NAMES);

const HAST_STUB_DESCRIPTORS: (readonly StubDescriptorEntry[] | undefined)[] = [];
for (const tag of Object.keys(HAST_STUB_FIELDS)) {
  const nodeType = Number(tag);
  const fields = [...HAST_STUB_FIELDS[nodeType]!];
  if (HAST_CONTAINER_TYPES.has(nodeType)) fields.push("children");
  HAST_STUB_DESCRIPTORS[nodeType] = stubDescriptors(fields);
}

/** Unknown node types still expose the prelude-backed lazy fields. */
const FALLBACK_DESCRIPTORS = stubDescriptors([]);

/**
 * Walk-path child stub: arena id + `type` eagerly, every other field a lazy
 * forward to the materialized node (first read snapshots the arena via
 * `materializeOne`, which enforces the handle epoch). Spread/identity rules
 * are enforced by `nid()` in hast-visitor.ts.
 */
export class HastChildStub {
  _resolver: HastResolver;
  _id: number;
  type: string;

  constructor(resolver: HastResolver, id: number, nodeType: number) {
    this._resolver = resolver;
    this._id = id;
    this.type = TYPE_NAME_BY_TAG[nodeType] ?? `unknown(${nodeType})`;
    installStubDescriptors(this, HAST_STUB_DESCRIPTORS[nodeType] ?? FALLBACK_DESCRIPTORS);
  }
}
