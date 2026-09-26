import type { List, ListItem, Root, Table } from "mdast";
import type {
  ArenaWire,
  Custom,
  DescriptionDetails,
  ContainerDirective,
  MdastNode,
  MdxJsxFlowElement,
} from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import { LEAF_TYPES, NAME_TO_TYPE, TYPE_NAMES } from "./generated/node-types.js";
import { createMaterializer, installNodeData } from "../materializer-cache.js";
import { FIELD, W_CHILDREN_COUNT, W_CHILDREN_START } from "../generated/arena-layout.js";
import { readMdastWireNode } from "../generated/fused-wire.js";

const MDAST_CUSTOM = NAME_TO_TYPE.custom!;

export { LEAF_TYPES };

const IS_LEAF = new Uint8Array(256);
for (const t of LEAF_TYPES) IS_LEAF[t] = 1;

// Custom nodes determine leafness from value, children, and data.h* rather than their type tag.
export function isCustomLeaf(
  node: { readonly value?: unknown; readonly data?: unknown },
  childCount: number,
): boolean {
  const { value, data } = node;
  if (childCount !== 0 || typeof value !== "string" || value === "") return false;
  if (data === null || typeof data !== "object") return true;
  if ("hName" in data && typeof data.hName === "string") return false;
  if ("hChildren" in data && Array.isArray(data.hChildren)) return false;
  if (!("hProperties" in data)) return true;
  const props = data.hProperties;
  return props === null || typeof props !== "object" || Array.isArray(props);
}

function addTypeProperties(
  node: MdastNode,
  reader: MdastReader,
  nodeId: number,
  nodeType: number,
): void {
  switch (nodeType) {
    case MDAST_CUSTOM: {
      // Omit an empty custom value so parent nodes do not masquerade as text leaves.
      const n = node as Custom;
      n.type = reader.fieldString(nodeId, 0);
      const value = reader.fieldString(nodeId, 8);
      if (value !== "") n.value = value;
      break;
    }

    case 5: {
      const n = node as List;
      const ordered = reader.fieldU8(nodeId, 4, 0) !== 0;
      n.ordered = ordered;
      n.start = ordered ? reader.fieldU32(nodeId, 0, 0) : null;
      n.spread = reader.fieldU8(nodeId, 5, 0) !== 0;
      break;
    }

    case 6: {
      const n = node as ListItem;
      // checked: 0=unchecked, 1=checked, 2=not-a-task-item.
      const checked = reader.fieldU8(nodeId, 0, 2);
      n.spread = reader.fieldU8(nodeId, 1, 0) !== 0;
      n.checked = checked === 2 ? null : checked === 1;
      break;
    }

    case 37: {
      (node as DescriptionDetails).spread = reader.fieldU8(nodeId, 0, 0) !== 0;
      break;
    }

    case 21:
      (node as Table).align = reader.getTableAlign(nodeId);
      break;

    case 30:
    case 31:
    case 32: {
      const d = reader.getDirectiveData(nodeId);
      const n = node as ContainerDirective;
      n.name = d.name;
      n.attributes = d.attributes;
      break;
    }

    case 100:
    case 101: {
      const d = reader.getMdxJsxElementData(nodeId);
      const n = node as MdxJsxFlowElement;
      n.name = d.name;
      n.attributes = d.attributes;
      break;
    }

    default:
      break;
  }
}

const mdastMaterializer = createMaterializer<MdastReader, MdastNode>({
  label: "materializeNode",
  typeNames: TYPE_NAMES,
  hasChildren: (nodeType, node, reader, nodeId) =>
    nodeType === MDAST_CUSTOM
      ? !isCustomLeaf(node, reader.getChildrenCount(nodeId))
      : IS_LEAF[nodeType] === 0,
  populate: (node, reader, nodeId, nodeType) => {
    if (!readMdastWireNode(reader.getWire(), nodeId, nodeType, node)) {
      addTypeProperties(node, reader, nodeId, nodeType);
    }
  },
});

export const materializeNode = mdastMaterializer.node;

// Not a per-call closure: fresh closures restart type feedback per tree, pinning small-tree calls to the slow tiers.
function buildMdastFused(
  reader: MdastReader,
  wire: ArenaWire,
  nodeId: number,
  nodeType: number,
): MdastNode {
  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;
  // Unified's assertNode requires a plain-object prototype.
  const node = { type: typeName } as MdastNode;
  if (!readMdastWireNode(wire, nodeId, nodeType, node)) {
    addTypeProperties(node, reader, nodeId, nodeType);
  }
  return node;
}

/** Materialize the full tree from root (nodeId=0) in one eager pass, breadth-first over the parents still to fill. */
export function materializeMdastTree(reader: MdastReader): Root {
  const wire = reader.getWire();
  const { u8, u32, nodesB, nodesW, strideB, strideW, childrenW } = wire;
  const dataTable = reader.getNodeDataTable();

  const rootType = u8[nodesB + FIELD.node_type] ?? 0;
  const root = buildMdastFused(reader, wire, 0, rootType);
  if (dataTable !== null) {
    const raw = dataTable.get(0);
    if (raw !== undefined) installNodeData(root, raw, "materializeMdastTree", 0);
  }

  const parents: MdastNode[] = [root];
  const parentIds: number[] = [0];
  for (let p = 0; p < parentIds.length; p++) {
    const parent = parents[p];
    const parentId = parentIds[p];
    if (parent === undefined || parentId === undefined) {
      throw new Error(`materializeMdastTree: parent queue hole at ${p}`);
    }
    const w = nodesW + parentId * strideW;
    const count = u32[w + W_CHILDREN_COUNT] ?? 0;
    const cbase = childrenW + (u32[w + W_CHILDREN_START] ?? 0);
    const kids = new Array<MdastNode>(count);
    for (let i = 0; i < count; i++) {
      const childId = u32[cbase + i] ?? 0;
      const childType = u8[nodesB + childId * strideB + FIELD.node_type] ?? 0;
      const child = buildMdastFused(reader, wire, childId, childType);
      kids[i] = child;
      if (dataTable !== null) {
        const raw = dataTable.get(childId);
        if (raw !== undefined) installNodeData(child, raw, "materializeMdastTree", childId);
      }
      const isContainer =
        childType === MDAST_CUSTOM
          ? !isCustomLeaf(child, reader.getChildrenCount(childId))
          : IS_LEAF[childType] === 0;
      if (isContainer) {
        parents.push(child);
        parentIds.push(childId);
      }
    }
    (parent as { children?: MdastNode[] }).children = kids;
  }
  return root as Root;
}
