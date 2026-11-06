// Pure-JS wire fixtures let reader tests run without a native binding.

import {
  ARENA_FORMAT_VERSION,
  ARENA_MAGIC,
  HEADER,
  KIND_MDAST,
  OFFSET_UNIT_UTF16,
} from "../src/generated/arena-layout.js";

export interface TreeNode {
  type: string;
  children?: TreeNode[];
}

export function findByType(node: TreeNode, type: string): TreeNode | undefined {
  if (node.type === type) return node;
  for (const c of node.children ?? []) {
    const hit = findByType(c, type);
    if (hit) return hit;
  }
  return undefined;
}

export function collect<T extends TreeNode>(
  node: TreeNode,
  pred: (n: TreeNode) => n is T,
  out: T[] = [],
): T[] {
  if (pred(node)) out.push(node);
  if (node.children) for (const c of node.children) collect(c, pred, out);
  return out;
}

const NODE_STRUCT_SIZE = 52;
const HEADER_SIZE = Object.keys(HEADER).length * 4;

interface NodeSpec {
  id?: number;
  type?: number;
  parent?: number;
  startOffset?: number;
  endOffset?: number;
  startLine?: number;
  startColumn?: number;
  endLine?: number;
  endColumn?: number;
  childrenStart?: number;
  childrenCount?: number;
  dataOffset?: number;
  dataLen?: number;
}

export function buildTestBuffer({
  source,
  nodes,
  children,
  typeData,
}: {
  source: string;
  nodes: NodeSpec[];
  children: number[];
  typeData: Uint8Array;
}): ArrayBuffer {
  const stringPoolBytes = new TextEncoder().encode(source);

  const nodesBytes = nodes.length * NODE_STRUCT_SIZE;
  const childrenBytes = children.length * 4;

  const nodesOffset = HEADER_SIZE;
  const childrenOffset = nodesOffset + nodesBytes;
  const typeDataOffset = childrenOffset + childrenBytes;
  const stringPoolOffset = typeDataOffset + typeData.length;
  const totalSize = stringPoolOffset + stringPoolBytes.length;

  const buf = new ArrayBuffer(totalSize);
  const view = new DataView(buf);
  const u8 = new Uint8Array(buf);

  view.setUint32(HEADER.magic, ARENA_MAGIC, true);
  view.setUint32(HEADER.kind, KIND_MDAST, true);
  view.setUint32(HEADER.format_version, ARENA_FORMAT_VERSION, true);
  view.setUint32(HEADER.offset_unit, OFFSET_UNIT_UTF16, true);
  view.setUint32(HEADER.node_struct_size, NODE_STRUCT_SIZE, true);
  view.setUint32(HEADER.node_count, nodes.length, true);
  view.setUint32(HEADER.nodes_offset, nodesOffset, true);
  view.setUint32(HEADER.children_count, children.length, true);
  view.setUint32(HEADER.children_offset, childrenOffset, true);
  view.setUint32(HEADER.type_data_len, typeData.length, true);
  view.setUint32(HEADER.type_data_offset, typeDataOffset, true);
  view.setUint32(HEADER.string_pool_len, stringPoolBytes.length, true);
  view.setUint32(HEADER.string_pool_offset, stringPoolOffset, true);
  view.setUint32(HEADER.source_len, source.length, true);
  view.setUint32(HEADER.node_data_offset, totalSize, true);

  for (let i = 0; i < nodes.length; i++) {
    const base = nodesOffset + i * NODE_STRUCT_SIZE;
    const n = nodes[i]!;
    view.setUint32(base + 0, n.id ?? i, true);
    view.setUint8(base + 4, n.type ?? 0);
    view.setUint32(base + 8, n.parent ?? 0, true);
    view.setUint32(base + 12, n.startOffset ?? 0, true);
    view.setUint32(base + 16, n.endOffset ?? 0, true);
    view.setUint32(base + 20, n.startLine ?? 1, true);
    view.setUint32(base + 24, n.startColumn ?? 1, true);
    view.setUint32(base + 28, n.endLine ?? 1, true);
    view.setUint32(base + 32, n.endColumn ?? 1, true);
    view.setUint32(base + 36, n.childrenStart ?? 0, true);
    view.setUint32(base + 40, n.childrenCount ?? 0, true);
    view.setUint32(base + 44, n.dataOffset ?? 0, true);
    view.setUint32(base + 48, n.dataLen ?? 0, true);
  }

  for (let i = 0; i < children.length; i++) {
    view.setUint32(childrenOffset + i * 4, children[i]!, true);
  }

  u8.set(typeData, typeDataOffset);

  u8.set(stringPoolBytes, stringPoolOffset);

  return buf;
}

// Type-data blobs must remain 4-byte aligned to match the arena writer.

export function buildHelloWorldBuffer(): ArrayBuffer {
  const source = "# Hello\n\nWorld";

  const typeData = new Uint8Array([1, 0, 0, 0, 2, 0, 0, 0, 5, 0, 0, 0, 9, 0, 0, 0, 5, 0, 0, 0]);

  return buildTestBuffer({
    source,
    nodes: [
      {
        id: 0,
        type: 0,
        parent: 0,
        startOffset: 0,
        endOffset: 14,
        startLine: 1,
        startColumn: 1,
        endLine: 2,
        endColumn: 6,
        childrenStart: 0,
        childrenCount: 2,
        dataOffset: 0,
        dataLen: 0,
      },
      {
        id: 1,
        type: 2,
        parent: 0,
        startOffset: 0,
        endOffset: 7,
        startLine: 1,
        startColumn: 1,
        endLine: 1,
        endColumn: 8,
        childrenStart: 2,
        childrenCount: 1,
        dataOffset: 0,
        dataLen: 1,
      },
      {
        id: 2,
        type: 1,
        parent: 0,
        startOffset: 9,
        endOffset: 14,
        startLine: 2,
        startColumn: 1,
        endLine: 2,
        endColumn: 6,
        childrenStart: 3,
        childrenCount: 1,
        dataOffset: 0,
        dataLen: 0,
      },
      {
        id: 3,
        type: 10,
        parent: 1,
        startOffset: 2,
        endOffset: 7,
        startLine: 1,
        startColumn: 3,
        endLine: 1,
        endColumn: 8,
        childrenStart: 0,
        childrenCount: 0,
        dataOffset: 4,
        dataLen: 8,
      },
      {
        id: 4,
        type: 10,
        parent: 2,
        startOffset: 9,
        endOffset: 14,
        startLine: 2,
        startColumn: 1,
        endLine: 2,
        endColumn: 6,
        childrenStart: 0,
        childrenCount: 0,
        dataOffset: 12,
        dataLen: 8,
      },
    ],
    children: [1, 2, 3, 4],
    typeData,
  });
}
