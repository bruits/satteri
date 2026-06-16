import { test, expect } from "vitest";
import { MdastReader, NodeType, NodeTypeName } from "../src/mdast/mdast-reader.js";
import { buildHelloWorldBuffer, buildTestBuffer } from "./fixtures.js";

test("NodeType constants", () => {
  expect(NodeType.Root).toBe(0);
  expect(NodeType.Heading).toBe(2);
  expect(NodeType.Text).toBe(10);
  expect(NodeType.Yaml).toBe(25);
  expect(NodeType.Toml).toBe(26);
  expect(NodeType.Math).toBe(27);
  expect(NodeType.InlineMath).toBe(28);
  expect(NodeTypeName[0]).toBe("Root");
  expect(NodeTypeName[2]).toBe("Heading");
  expect(NodeTypeName[10]).toBe("Text");
  expect(NodeTypeName[25]).toBe("Yaml");
});

test("MdastReader rejects invalid magic", () => {
  const buf = new ArrayBuffer(44);
  expect(() => new MdastReader(buf)).toThrow(/bad magic/);
});

test("MdastReader rejects wrong kind", () => {
  const buf = new ArrayBuffer(44);
  const view = new DataView(buf);
  view.setUint32(0, 0x5241444d, true); // correct "MDAR" magic
  view.setUint32(4, 2, true); // KIND_HAST = 2, but MdastReader expects 1
  expect(() => new MdastReader(buf)).toThrow(/kind/);
});

test("MdastReader reads node count", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  expect(reader.nodeCount).toBe(5);
});

test("MdastReader reads root node", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  const root = reader.getNode(0);
  expect(root.type).toBe(NodeType.Root);
  expect(root.typeName).toBe("Root");
  expect(root.childrenCount).toBe(2);
  expect(root.position!.start.line).toBe(1);
  expect(root.position!.start.column).toBe(1);
});

test("MdastReader reads heading node", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  const heading = reader.getNode(1);
  expect(heading.type).toBe(NodeType.Heading);
  expect(heading.childrenCount).toBe(1);
  // depth is the first byte of HeadingData (the generated decoder reads it the same way)
  expect(reader.getTypeData(1)[0]).toBe(1);
});

test("MdastReader reads text values", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  expect(reader.getTextValue(3)).toBe("Hello");
  expect(reader.getTextValue(4)).toBe("World");
});

test("MdastReader getNodeType fast path", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  expect(reader.getNodeType(0)).toBe(NodeType.Root);
  expect(reader.getNodeType(1)).toBe(NodeType.Heading);
  expect(reader.getNodeType(2)).toBe(NodeType.Paragraph);
  expect(reader.getNodeType(3)).toBe(NodeType.Text);
});

test("MdastReader getChildIds", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  expect(reader.getChildIds(0)).toEqual([1, 2]);
  expect(reader.getChildIds(1)).toEqual([3]);
  expect(reader.getChildIds(2)).toEqual([4]);
  expect(reader.getChildIds(3)).toEqual([]);
});

test("MdastReader.walk visits all nodes", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  const visited: { nodeId: number; nodeType: number }[] = [];
  reader.walk((nodeId, nodeType) => {
    visited.push({ nodeId, nodeType });
  });
  expect(visited.length).toBe(5);
  expect(visited[0]!.nodeId).toBe(0);
});

test("MdastReader.walk skip children", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  const visited: number[] = [];
  reader.walk((nodeId, nodeType) => {
    visited.push(nodeId);
    if (nodeType === NodeType.Heading) return false;
  });
  expect(visited).toContain(0);
  expect(visited).toContain(1);
  expect(visited).not.toContain(3); // Text "Hello" skipped
  expect(visited).toContain(2);
  expect(visited).toContain(4);
});

test("MdastReader getSource", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  expect(reader.getSource()).toBe("# Hello\n\nWorld");
});

test("MdastReader accepts Uint8Array", () => {
  const buf = buildHelloWorldBuffer();
  const u8 = new Uint8Array(buf);
  const reader = new MdastReader(u8);
  expect(reader.nodeCount).toBe(5);
  expect(reader.getTextValue(3)).toBe("Hello");
});

test("MdastReader getTypeData returns empty for nodes without data", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  expect(reader.getTypeData(2).length).toBe(0); // Paragraph has no data
});

test("MdastReader out of range node throws", () => {
  const buf = buildHelloWorldBuffer();
  const reader = new MdastReader(buf);
  expect(() => reader.getNode(99)).toThrow(/out of range/);
});

test("getListData reads correct layout: start(u32)@0, ordered(bool)@4, spread(bool)@5", () => {
  // Build a minimal list buffer to verify layout offsets
  // ListData #[repr(C)]: start(0..4), ordered(4), spread(5), _pad(6..8)
  const typeData = new Uint8Array(8);
  const view = new DataView(typeData.buffer);
  view.setUint32(0, 42, true); // start = 42
  typeData[4] = 1; // ordered = true
  typeData[5] = 1; // spread = true

  const buf = buildTestBuffer({
    source: "",
    nodes: [
      { id: 0, type: 0, childrenStart: 0, childrenCount: 1, dataOffset: 0, dataLen: 0 },
      { id: 1, type: 5, childrenStart: 0, childrenCount: 0, dataOffset: 0, dataLen: 8 },
    ],
    children: [1],
    typeData,
  });
  const reader = new MdastReader(buf);
  const d = reader.getListData(1);
  expect(d.start).toBe(42);
  expect(d.ordered).toBe(true);
  expect(d.spread).toBe(true);
});
