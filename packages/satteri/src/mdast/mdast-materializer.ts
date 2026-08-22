import type { Root } from "mdast";
import type { Position } from "unist";
import type { MdastNode } from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import { NAME_TO_TYPE, TYPE_NAMES } from "./generated/node-types.js";
import { materializeMdastFields } from "./generated/layout.js";
import { createMaterializer, installNodeData } from "../materializer-cache.js";
import { FIELD } from "../generated/arena-layout.js";

/** Internal tag for user-defined nodes; its stored `name` field carries the
 *  author's public `type` string. */
const MDAST_CUSTOM = NAME_TO_TYPE.custom!;

// Leaf node types that do NOT have children.
// Type 9 = `definition`; type 18 = `imageReference` — leaves per mdast spec
// (imageReference carries `alt` as a string, not children).
export const LEAF_TYPES: ReadonlySet<number> = new Set([
  9, 10, 13, 7, 8, 14, 3, 16, 18, 20, 25, 26, 27, 28, 102, 103, 104,
]);

const IS_LEAF = new Uint8Array(256);
for (const t of LEAF_TYPES) IS_LEAF[t] = 1;

/** A `custom` node is a leaf when it has a non-empty `value` and no children or
 *  `data.h*`. Leafness is per node there, not per type, so the read paths ask
 *  this instead of {@link LEAF_TYPES}.
 *  @see {@link Custom} */
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

/**
 * Add type-specific properties to a node object as eager plain stores.
 */
function addTypeProperties(
  node: MdastNode,
  reader: MdastReader,
  nodeId: number,
  nodeType: number,
): void {
  // Fixed-field types materialize from the generated layout table; the rest
  // (variable-length / cross-field) stay in the hand-written switch.
  if (materializeMdastFields(reader, node, nodeId, nodeType)) {
    // User-defined node: surface the stored `name` as the open `node.type`.
    // Drop an empty `value` so a parent node isn't given a spurious leaf field.
    if (nodeType === MDAST_CUSTOM) {
      const n = node as { type: string; name?: string; value?: string };
      if (n.name !== undefined) n.type = n.name;
      delete n.name;
      if (n.value === "") delete n.value;
    }
    return;
  }

  switch (nodeType) {
    case 5: {
      // list
      const n = node as { ordered: boolean; start: number | null; spread: boolean };
      const ordered = reader.fieldU8(nodeId, 4, 0) !== 0;
      n.ordered = ordered;
      n.start = ordered ? reader.fieldU32(nodeId, 0, 0) : null;
      n.spread = reader.fieldU8(nodeId, 5, 0) !== 0;
      break;
    }

    case 6: {
      // listItem
      const n = node as { spread: boolean; checked: boolean | null };
      // checked: 0=unchecked, 1=checked, 2=not-a-task-item.
      const checked = reader.fieldU8(nodeId, 0, 2);
      n.spread = reader.fieldU8(nodeId, 1, 0) !== 0;
      n.checked = checked === 2 ? null : checked === 1;
      break;
    }

    case 37: {
      // descriptionDetails
      (node as { spread: boolean }).spread = reader.fieldU8(nodeId, 0, 0) !== 0;
      break;
    }

    case 21: // table
      (node as { align: unknown }).align = reader.getTableAlign(nodeId);
      break;

    case 30: // containerDirective
    case 31: // leafDirective
    case 32: {
      // textDirective
      const d = reader.getDirectiveData(nodeId);
      const n = node as { name: string; attributes: unknown };
      n.name = d.name;
      n.attributes = d.attributes;
      break;
    }

    case 100: // mdxJsxFlowElement
    case 101: {
      // mdxJsxTextElement
      const d = reader.getMdxJsxElementData(nodeId);
      const n = node as { name: string | null; attributes: unknown };
      n.name = d.name;
      n.attributes = d.attributes;
      break;
    }

    // Nodes with no type-specific props:
    // root(0), paragraph(1), thematicBreak(3), blockquote(4),
    // emphasis(11), strong(12), break(14), tableRow(22), tableCell(23), delete(24)
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
  populate: addTypeProperties,
});

/**
 * Materialize a single MDAST node; scalars eager, `children` lazy, memoized
 * per `(reader, id)`; `frozen` (the plugin walk path) deep-freezes so plugins
 * cannot corrupt the shared cache.
 */
export const materializeNode = mdastMaterializer.node;

const W_START_OFFSET = FIELD.start_offset >> 2;
const W_CHILDREN_START = FIELD.children_start >> 2;
const W_CHILDREN_COUNT = FIELD.children_count >> 2;
const W_DATA_OFFSET = FIELD.data_offset >> 2;
const W_DATA_LEN = FIELD.data_len >> 2;

// Not a per-call closure: fresh closures restart type feedback per tree, pinning small-tree calls to the slow tiers.
function buildMdastFused(
  reader: MdastReader,
  u32: Uint32Array,
  nodesW: number,
  strideW: number,
  typeDataB: number,
  pool: string,
  hasNodeData: boolean,
  nodeId: number,
  nodeType: number,
): MdastNode {
  {
    const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;
    // Plain object, not a class: unified's `assertNode` rejects any other prototype.
    const node = { type: typeName } as unknown as MdastNode;
    const w = nodesW + nodeId * strideW + W_START_OFFSET;
    const startLine = u32[w + 2] ?? 0;
    if (startLine !== 0) {
      (node as { position?: Position }).position = {
        start: { offset: u32[w] ?? 0, line: startLine, column: u32[w + 3] ?? 0 },
        end: { offset: u32[w + 1] ?? 0, line: u32[w + 4] ?? 0, column: u32[w + 5] ?? 0 },
      };
    }
    switch (nodeType) {
      case 7:
      case 10:
      case 13:
      case 25:
      case 26: {
        const wd = nodesW + nodeId * strideW;
        let value = "";
        if ((u32[wd + W_DATA_LEN] ?? 0) >= 8) {
          const at = (typeDataB + (u32[wd + W_DATA_OFFSET] ?? 0)) >> 2;
          const off = u32[at] ?? 0;
          value = pool.substring(off, off + (u32[at + 1] ?? 0));
        }
        (node as { value: string }).value = value;
        break;
      }
      default:
        addTypeProperties(node, reader, nodeId, nodeType);
        break;
    }
    if (hasNodeData) {
      installNodeData(node, reader.getNodeData(nodeId), "materializeMdastTree", nodeId);
    }
    return node;
  }
}

/** Materialize the full tree from root (nodeId=0), fused: one-shot walk, so wire loads replace per-node reader calls. */
export function materializeMdastTree(reader: MdastReader): Root {
  const { u8, u32, nodesB, nodesW, strideB, strideW, childrenW, typeDataB, pool } =
    reader.getWire();
  const hasNodeData = reader.hasNodeData();

  const rootType = u8[nodesB + FIELD.node_type] ?? 0;
  const root = buildMdastFused(
    reader, u32, nodesW, strideW, typeDataB, pool, hasNodeData, 0, rootType,
  );

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
      const child = buildMdastFused(
        reader, u32, nodesW, strideW, typeDataB, pool, hasNodeData, childId, childType,
      );
      kids[i] = child;
      const isContainer =
        childType === MDAST_CUSTOM
          ? !isCustomLeaf(child, u32[nodesW + childId * strideW + W_CHILDREN_COUNT] ?? 0)
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
