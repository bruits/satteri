// Builds a minimal valid MDAST buffer in pure JS for testing MdastReader
// without requiring the native module to be built.

const MAGIC = 0x5241444d; // "MDAR" bytes [0x4d,0x44,0x41,0x52] read as little-endian u32
const NODE_STRUCT_SIZE = 52;
const HEADER_SIZE = 44;

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
  const sourceBytes = new TextEncoder().encode(source);

  const nodesBytes = nodes.length * NODE_STRUCT_SIZE;
  const childrenBytes = children.length * 4;

  const nodesOffset = HEADER_SIZE;
  const childrenOffset = nodesOffset + nodesBytes;
  const typeDataOffset = childrenOffset + childrenBytes;
  const sourceOffset = typeDataOffset + typeData.length;
  const totalSize = sourceOffset + sourceBytes.length;

  const buf = new ArrayBuffer(totalSize);
  const view = new DataView(buf);
  const u8 = new Uint8Array(buf);

  // Header
  view.setUint32(0, MAGIC, true);
  view.setUint32(4, 1, true);
  view.setUint32(8, NODE_STRUCT_SIZE, true);
  view.setUint32(12, nodes.length, true);
  view.setUint32(16, nodesOffset, true);
  view.setUint32(20, children.length, true);
  view.setUint32(24, childrenOffset, true);
  view.setUint32(28, typeData.length, true);
  view.setUint32(32, typeDataOffset, true);
  view.setUint32(36, sourceBytes.length, true);
  view.setUint32(40, sourceOffset, true);

  // Nodes
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

  // Children
  for (let i = 0; i < children.length; i++) {
    view.setUint32(childrenOffset + i * 4, children[i]!, true);
  }

  // Type data
  u8.set(typeData, typeDataOffset);

  // Source
  u8.set(sourceBytes, sourceOffset);

  return buf;
}

export const NodeType = {
  Root: 0,
  Paragraph: 1,
  Heading: 2,
  Text: 10,
  Link: 15,
} as const;

// A simple "# Hello\n\nWorld" arena
// source = "# Hello\n\nWorld"
//   Root (id=0, children=[1,2])
//   Heading depth=1 (id=1, children=[3], parent=0)
//   Paragraph (id=2, children=[4], parent=0)
//   Text "Hello" (id=3, parent=1, StringRef offset=2 len=5)
//   Text "World" (id=4, parent=2, StringRef offset=9 len=5)
// typeData: [1] (HeadingData.depth) then [2,0,0,0, 5,0,0,0] (StringRef Hello) then [9,0,0,0, 5,0,0,0] (StringRef World)
// children array: [1, 2, 3, 4]

export function buildHelloWorldBuffer(): ArrayBuffer {
  const source = "# Hello\n\nWorld";

  const typeData = new Uint8Array([
    1, // HeadingData.depth = 1
    2,
    0,
    0,
    0,
    5,
    0,
    0,
    0, // StringRef for "Hello": offset=2, len=5
    9,
    0,
    0,
    0,
    5,
    0,
    0,
    0, // StringRef for "World": offset=9, len=5
  ]);

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
        dataOffset: 1,
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
        dataOffset: 9,
        dataLen: 8,
      },
    ],
    children: [1, 2, 3, 4],
    typeData,
  });
}
