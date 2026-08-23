import {
  HastReader,
  HAST_ROOT,
  HAST_ELEMENT,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
} from "./hast-reader.js";
import type { Root } from "hast";
import type { ArenaWire, HastNode } from "../types.js";
import { TYPE_NAMES } from "./generated/node-types.js";
import { createMaterializer, installNodeData } from "../materializer-cache.js";
import { FIELD, W_CHILDREN_COUNT, W_CHILDREN_START } from "../generated/arena-layout.js";
import { readHastWireNode } from "../generated/fused-wire.js";

export type { HastNode };

/** Container node types (the ones that carry `children`); everything else is a leaf. */
export const HAST_CONTAINER_TYPES: ReadonlySet<number> = new Set([
  HAST_ROOT,
  HAST_ELEMENT,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
]);

const IS_CONTAINER = new Uint8Array(256);
for (const t of HAST_CONTAINER_TYPES) IS_CONTAINER[t] = 1;

const hastMaterializer = createMaterializer<HastReader, HastNode>({
  label: "materializeHastNode",
  typeNames: TYPE_NAMES,
  hasChildren: (nodeType) => IS_CONTAINER[nodeType] === 1,
  populate(node, reader, nodeId, nodeType) {
    if (!readHastWireNode(reader.getWire(), nodeId, nodeType, node)) {
      addHastTypeProperties(node, reader, nodeId, nodeType);
    }
  },
});

/**
 * Materialize a single HAST node; scalars eager, `children` lazy, memoized per
 * `(reader, id)`; `frozen` (the plugin walk path) deep-freezes so plugins
 * cannot corrupt the shared cache.
 */
export const materializeHastNode = hastMaterializer.node;

/** The reader-path decode for the tags `readHastWireNode` hands back: MDX JSX
 *  elements, whose kind-tagged attribute assembly stays on the reader. */
function addHastTypeProperties(
  node: HastNode,
  reader: HastReader,
  nodeId: number,
  nodeType: number,
): void {
  if (nodeType === HAST_MDX_JSX_ELEMENT || nodeType === HAST_MDX_JSX_TEXT_ELEMENT) {
    const { name, attributes } = reader.getMdxJsxElementData(nodeId);
    (node as { name: string | null }).name = name;
    (node as { attributes: unknown }).attributes = attributes;
  }
}

// Not a per-call closure: fresh closures restart type feedback per tree, pinning small-tree calls to the slow tiers.
function buildHastFused(
  reader: HastReader,
  wire: ArenaWire,
  nodeId: number,
  nodeType: number,
): HastNode {
  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;
  // Plain object, not a class: unified's `assertNode` rejects any other prototype.
  const node = { type: typeName } as unknown as HastNode;
  if (!readHastWireNode(wire, nodeId, nodeType, node)) {
    addHastTypeProperties(node, reader, nodeId, nodeType);
  }
  return node;
}

/** Materialize the full HAST tree from root (nodeId=0) in one eager pass, breadth-first over the parents still to fill. */
export function materializeHastTree(reader: HastReader): Root {
  const wire = reader.getWire();
  const { u8, u32, nodesB, nodesW, strideB, strideW, childrenW } = wire;
  const dataTable = reader.getNodeDataTable();

  const rootType = u8[nodesB + FIELD.node_type] ?? 0;
  const root = buildHastFused(reader, wire, 0, rootType);
  if (dataTable !== null) {
    const raw = dataTable.get(0);
    if (raw !== undefined) installNodeData(root, raw, "materializeHastTree", 0);
  }
  if (IS_CONTAINER[rootType] !== 1) return root as Root;

  const parents: HastNode[] = [root];
  const parentIds: number[] = [0];
  for (let p = 0; p < parentIds.length; p++) {
    const parent = parents[p];
    const parentId = parentIds[p];
    if (parent === undefined || parentId === undefined) {
      throw new Error(`materializeHastTree: parent queue hole at ${p}`);
    }
    const w = nodesW + parentId * strideW;
    const count = u32[w + W_CHILDREN_COUNT] ?? 0;
    const cbase = childrenW + (u32[w + W_CHILDREN_START] ?? 0);
    const kids = new Array<HastNode>(count);
    for (let i = 0; i < count; i++) {
      const childId = u32[cbase + i] ?? 0;
      const childType = u8[nodesB + childId * strideB + FIELD.node_type] ?? 0;
      const child = buildHastFused(reader, wire, childId, childType);
      kids[i] = child;
      if (dataTable !== null) {
        const raw = dataTable.get(childId);
        if (raw !== undefined) installNodeData(child, raw, "materializeHastTree", childId);
      }
      if (IS_CONTAINER[childType] === 1) {
        parents.push(child);
        parentIds.push(childId);
      }
    }
    (parent as { children?: HastNode[] }).children = kids;
  }
  return root as Root;
}
