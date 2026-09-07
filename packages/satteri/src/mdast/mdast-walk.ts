import type { AnyHandle } from "../handles.js";
import {
  LazyChildResolver,
  registerEpochCacheSlot,
  type EpochCache,
} from "../lazy-child-resolver.js";
import type { MdastNode } from "../types.js";
import { crossPipelineForeign, FOREIGN_REF, type NodeRefs } from "../visitor-shared.js";
import { readPosition, rstr, ru32 } from "../wire-read.js";
import { MdastChildStub } from "./child-stub.js";
import { decodeMdastTypeData } from "./generated/layout.js";
import { NAME_TO_TYPE, TYPE_NAMES } from "./generated/node-types.js";
import { isCustomLeaf, LEAF_TYPES, materializeNode } from "./mdast-materializer.js";
import { MdastReader } from "./mdast-reader.js";

const MDAST_CUSTOM = NAME_TO_TYPE.custom!;

export function getNodeId(node: MdastNode, refs: NodeRefs): number | undefined {
  // Spread copies must be treated as new content, not as references to the original arena node.
  if (node instanceof MdastChildStub) return node._refs === refs ? node._id : FOREIGN_REF;
  const id = refs.get(node);
  if (id !== undefined) return id;
  // An unregistered non-enumerable ID belongs to a different tree; enumerable copies carry no identity.
  const d = Object.getOwnPropertyDescriptor(node, "_nodeId");
  if (d !== undefined && !d.enumerable) return FOREIGN_REF;
  return crossPipelineForeign(node);
}

const MDAST_EPOCH_CACHE = registerEpochCacheSlot(new WeakMap<AnyHandle, EpochCache<MdastReader>>());

export class MdastLazyChildResolver extends LazyChildResolver<MdastReader, MdastNode> {
  protected override cacheSlot() {
    return MDAST_EPOCH_CACHE;
  }

  protected override createReader(wire: Uint8Array): MdastReader {
    return new MdastReader(wire);
  }

  protected override materializeNode(
    reader: MdastReader,
    nodeId: number,
    refs: NodeRefs,
  ): MdastNode {
    return materializeNode(reader, nodeId, true, refs);
  }

  protected override readParentId(reader: MdastReader, nodeId: number): number {
    return reader.getParentId(nodeId);
  }

  protected override readChildIds(reader: MdastReader, nodeId: number): number[] {
    return reader.getChildIds(nodeId);
  }
}

function readMdastChildStubs(
  view: DataView,
  buf: Uint8Array,
  idsPos: number,
  typesPos: number,
  count: number,
  resolver: MdastLazyChildResolver,
): MdastNode[] {
  // With a hot snapshot a stub's deferral buys nothing; real nodes skip its per-field getters.
  if (resolver.hasHotSnapshot()) {
    const nodes: MdastNode[] = new Array(count);
    for (let i = 0; i < count; i++) {
      nodes[i] = resolver.materializeOne(ru32(view, idsPos + i * 4));
    }
    return nodes;
  }
  const stubs: MdastNode[] = new Array(count);
  for (let i = 0; i < count; i++) {
    stubs[i] = new MdastChildStub(
      resolver,
      ru32(view, idsPos + i * 4),
      buf[typesPos + i]!,
    ) as unknown as MdastNode;
  }
  return stubs;
}

// Own enumerable getters preserve children in spreads; closures avoid costly per-node wire slots.
function makeLazyChildren(
  node: object,
  view: DataView,
  buf: Uint8Array,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  resolver: MdastLazyChildResolver,
): void {
  Object.defineProperty(node, "children", {
    get(this: object): MdastNode[] {
      const val = readMdastChildStubs(view, buf, childIdsPos, childTypesPos, childCount, resolver);
      Object.defineProperty(this, "children", {
        value: val,
        writable: true,
        enumerable: true,
        configurable: true,
      });
      return val;
    },
    enumerable: true,
    configurable: true,
  });
}

export function readMdastMatchedNode(
  view: DataView,
  buf: Uint8Array,
  dataOffset: number,
  nodeId: number,
  nodeType: number,
  resolver: MdastLazyChildResolver,
): MdastNode {
  let pos = dataOffset;

  const dataJsonLen = ru32(view, pos);
  pos += 4;
  let initialData: Record<string, unknown> | null = null;
  if (dataJsonLen > 0) {
    const jsonStr = rstr(buf, pos, dataJsonLen);
    try {
      initialData = JSON.parse(jsonStr);
    } catch (err) {
      if (process.env.NODE_ENV !== "production") {
        console.warn(`readMdastMatchedNode: malformed node_data for nodeId=${nodeId}`, err);
      }
    }
    pos += dataJsonLen;
  }

  const position = readPosition(view, pos);
  pos += 24;

  const childCount = ru32(view, pos);
  pos += 4;
  // Most visitors never read children, so defer decoding their IDs and types.
  const childIdsPos = pos;
  pos += childCount * 4;
  const childTypesPos = pos;
  pos += childCount;

  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  const node: Record<string, unknown> = { type: typeName };
  if (position !== undefined) node.position = position;
  if (childCount > 0) {
    makeLazyChildren(node, view, buf, childIdsPos, childTypesPos, childCount, resolver);
  }

  if (!decodeMdastTypeData(view, buf, pos, nodeType, node)) {
    switch (nodeType) {
      case 5: {
        node.start = ru32(view, pos);
        node.ordered = buf[pos + 4]! !== 0;
        node.spread = buf[pos + 5]! !== 0;
        if (!node.ordered) node.start = null;
        break;
      }
      case 6: {
        const checked = buf[pos]!;
        node.checked = checked === 2 ? null : checked === 1;
        node.spread = buf[pos + 1]! !== 0;
        break;
      }
      case 37: {
        node.spread = buf[pos]! !== 0;
        break;
      }
    }
  }

  // Custom nodes expose name as type; an empty value must not make a parent look like a text leaf.
  if (nodeType === MDAST_CUSTOM) {
    node.type = node.name as string;
    delete node.name;
    if (node.value === "") delete node.value;
  }

  const leafCustom =
    nodeType === MDAST_CUSTOM && isCustomLeaf({ value: node.value, data: initialData }, childCount);

  if (childCount === 0 && !LEAF_TYPES.has(nodeType) && !leafCustom) {
    node.children = [];
  }

  resolver.refs.set(node, nodeId);

  if (initialData) {
    node.data = initialData;
  }

  return node as unknown as MdastNode;
}
