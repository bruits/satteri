import { materializeHastNode, type HastNode } from "./hast-materializer.js";
import type { HastNodeInternal, HastRaw, MdxJsxAttributeUnion } from "../types.js";
import type { Element, Text, Comment, Doctype, Root } from "hast";
import type { MdxJsxFlowElementHast, MdxJsxTextElementHast } from "mdast-util-mdx-jsx";
import type { MdxFlowExpressionHast, MdxTextExpressionHast } from "mdast-util-mdx-expression";
import type { MdxjsEsmHast } from "mdast-util-mdxjs-esm";
import {
  HastReader,
  HAST_ROOT,
  HAST_ELEMENT,
  HAST_TEXT,
  HAST_COMMENT,
  HAST_RAW,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
  HAST_MDX_FLOW_EXPRESSION,
  HAST_MDX_TEXT_EXPRESSION,
  HAST_MDX_ESM,
  type HastProperty,
} from "./hast-reader.js";
import { CommandBuffer } from "../command-buffer.js";
import { DataMap } from "../data-map.js";
import { walkHandle, applyCommandsToHandle, serializeHandle } from "../../index.js";

// Opaque handle type from NAPI — the arena lives in Rust memory.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type HastHandle = any;

export interface HastDiagnostic {
  message: string;
  nodeId?: number | undefined;
  severity: "error" | "warning" | "info";
}

export interface HastVisitorContext {
  removeNode(node: HastNode): void;
  replaceNode(node: HastNode, newNode: HastNode): void;
  setProperty(node: HastNode, key: string, value: unknown): void;
  report(opts: { message: string; node?: HastNode; severity?: "error" | "warning" | "info" }): void;
  getDiagnostics(): HastDiagnostic[];
}

function isChildRefArray(children: unknown): boolean {
  if (!Array.isArray(children) || children.length === 0) return false;
  return children.every((c: Record<string, unknown>) => c?.type === "__child_ref__");
}

/** Inject `_hast: true` marker on a HastNode and all its children for JSON serialization. */
function markHast(node: HastNode): Record<string, unknown> {
  const n = node as unknown as Record<string, unknown>;
  const obj: Record<string, unknown> = { _hast: true, type: node.type };
  if ("tagName" in node) obj.tagName = n.tagName;
  if ("properties" in node) obj.properties = n.properties;
  if ("value" in node) obj.value = n.value;
  if ("name" in node) obj.name = n.name;
  if ("attributes" in node) obj.attributes = n.attributes;
  if ("children" in node && isChildRefArray(n.children)) {
    obj._keepChildren = true;
  } else if ("children" in node) {
    obj.children = (n.children as HastNode[]).map(markHast);
  }
  return obj;
}

function nid(node: HastNode): number {
  return (node as HastNodeInternal)._nodeId;
}

class HastVisitorContextImpl implements HastVisitorContext {
  readonly #commandBuffer: CommandBuffer = new CommandBuffer();
  readonly #diagnostics: HastDiagnostic[] = [];
  /** Track accumulated node state for multiple setProperty calls on the same node. */
  readonly #pendingNodes: Map<number, HastNode> = new Map();

  removeNode(node: HastNode): void {
    this.#commandBuffer.removeNode(nid(node));
  }

  replaceNode(node: HastNode, newNode: HastNode): void {
    const id = nid(node);
    // Encode as a REPLACE command with _hast marker for Rust deserialization
    this.#commandBuffer.replaceRawJson(id, JSON.stringify(markHast(newNode)));
    this.#pendingNodes.set(id, newNode);
  }

  setProperty(node: HastNode, key: string, value: unknown): void {
    const id = nid(node);
    if (node.type === "element") {
      // Fast binary path — no materialization, no JSON serialization
      this.#commandBuffer.setProperty(id, key, value);
      return;
    }

    if (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") {
      // MDX JSX nodes use `attributes`, not `properties` — keep replaceNode path
      const current = this.#pendingNodes.get(id) ?? node;
      const updated: Record<string, unknown> = { ...current };
      const attrs = [...((updated.attributes as MdxJsxAttributeUnion[] | undefined) ?? [])];
      const idx = attrs.findIndex((a) => a.type === "mdxJsxAttribute" && a.name === key);
      if (idx !== -1) attrs.splice(idx, 1);
      const attrValue =
        value === true || value === null || value === undefined
          ? null
          : typeof value === "string"
            ? value
            : `${value as string | number | boolean}`;
      attrs.push({ type: "mdxJsxAttribute", name: key, value: attrValue });
      updated.attributes = attrs;
      this.replaceNode(node, updated as unknown as HastNode);
      return;
    }

    // Fallback for other node types
    const current = this.#pendingNodes.get(id) ?? node;
    const updated: Record<string, unknown> = { ...current };
    const props = (updated.properties ?? {}) as Record<string, string | boolean | string[]>;
    updated.properties = { ...props, [key]: value as string | boolean | string[] };
    this.replaceNode(node, updated as unknown as HastNode);
  }

  report({
    message,
    node,
    severity = "error",
  }: {
    message: string;
    node?: HastNode;
    severity?: "error" | "warning" | "info";
  }): void {
    this.#diagnostics.push({ message, nodeId: node ? nid(node) : undefined, severity });
  }

  getCommandBuffer(): CommandBuffer {
    return this.#commandBuffer;
  }

  getDiagnostics(): HastDiagnostic[] {
    return this.#diagnostics;
  }
}

/** A filtered visitor: Rust filters by tag/component name, only matched nodes cross the boundary. */
export interface HastFilteredVisitor<N extends HastNode = HastNode> {
  filter: string[];
  visit(node: N, ctx: HastVisitorContext): HastNode | void;
}

type HastVisitorFn<N extends HastNode = HastNode> = (
  node: N,
  ctx: HastVisitorContext,
) => HastNode | void;

export interface HastVisitorInstance {
  before?(ctx: HastVisitorContext): void;
  after?(ctx: HastVisitorContext): void;
  transformRoot?(root: Root, ctx: HastVisitorContext): HastNode | void;
  // Element-like nodes: filtered by tag/component name (single or array)
  element?: HastFilteredVisitor<Element> | HastFilteredVisitor<Element>[];
  mdxJsxFlowElement?:
    | HastFilteredVisitor<MdxJsxFlowElementHast>
    | HastFilteredVisitor<MdxJsxFlowElementHast>[];
  mdxJsxTextElement?:
    | HastFilteredVisitor<MdxJsxTextElementHast>
    | HastFilteredVisitor<MdxJsxTextElementHast>[];
  // Leaf/value nodes: bare functions (no tag names to filter on)
  text?: HastVisitorFn<Text>;
  comment?: HastVisitorFn<Comment>;
  raw?: HastVisitorFn<HastRaw>;
  doctype?: HastVisitorFn<Doctype>;
  mdxFlowExpression?: HastVisitorFn<MdxFlowExpressionHast>;
  mdxTextExpression?: HastVisitorFn<MdxTextExpressionHast>;
  mdxjsEsm?: HastVisitorFn<MdxjsEsmHast>;
}

export interface HastVisitResult {
  commandBuffer: Uint8Array;
  diagnostics: HastDiagnostic[];
  hasMutations: boolean;
}

// ---------------------------------------------------------------------------
// Lightweight node materializer for the visitor hot path.
//
// Avoids per-node Object.defineProperty calls by using class prototypes
// with lazy getters that cache on first access.
// ---------------------------------------------------------------------------

function propsToRecord(props: HastProperty[]): Record<string, string | boolean | string[]> {
  const result: Record<string, string | boolean | string[]> = {};
  for (const p of props) {
    result[p.name] = p.value;
  }
  return result;
}

class LazyElementNode {
  type = "element" as const;
  _nodeId: number;
  declare _reader: HastReader;
  declare _dataMap: DataMap;
  declare tagName: string;
  declare properties: Record<string, string | boolean | string[]>;
  declare children: HastNode[];
  declare data: Record<string, unknown> | null;

  constructor(nodeId: number, reader: HastReader, dataMap: DataMap) {
    this._nodeId = nodeId;
    // Store reader/dataMap as non-enumerable to avoid serialization
    Object.defineProperty(this, "_reader", { value: reader, enumerable: false });
    Object.defineProperty(this, "_dataMap", { value: dataMap, enumerable: false });
  }
}

// Helper: resolve element data once and cache both tagName and properties.
function resolveElementData(self: LazyElementNode): void {
  const { tagName, properties } = self._reader.getElementData(self._nodeId);
  Object.defineProperty(self, "tagName", {
    value: tagName,
    writable: true,
    enumerable: true,
    configurable: true,
  });
  Object.defineProperty(self, "properties", {
    value: propsToRecord(properties),
    writable: true,
    enumerable: true,
    configurable: true,
  });
}

// Define lazy getters on prototype — one-time cost at module load
Object.defineProperty(LazyElementNode.prototype, "tagName", {
  get(this: LazyElementNode) {
    resolveElementData(this);
    return this.tagName;
  },
  configurable: true,
  enumerable: true,
});

Object.defineProperty(LazyElementNode.prototype, "properties", {
  get(this: LazyElementNode) {
    resolveElementData(this);
    return this.properties;
  },
  configurable: true,
  enumerable: true,
});

Object.defineProperty(LazyElementNode.prototype, "children", {
  get(this: LazyElementNode) {
    const ids = this._reader.getChildIds(this._nodeId);
    const val = ids.map((id) => materializeHastNode(this._reader, id, this._dataMap));
    Object.defineProperty(this, "children", {
      value: val,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    return val;
  },
  configurable: true,
  enumerable: true,
});

Object.defineProperty(LazyElementNode.prototype, "data", {
  get(this: LazyElementNode) {
    return this._dataMap.get(this._nodeId);
  },
  set(this: LazyElementNode, value: Record<string, unknown>) {
    this._dataMap.set(this._nodeId, value);
  },
  configurable: true,
  enumerable: true,
});

class LazyTextNode {
  _nodeId: number;
  declare _reader: HastReader;
  declare _dataMap: DataMap;
  declare value: string;
  declare data: Record<string, unknown> | null;

  type: string;
  constructor(type: string, nodeId: number, reader: HastReader, dataMap: DataMap) {
    this.type = type;
    this._nodeId = nodeId;
    Object.defineProperty(this, "_reader", { value: reader, enumerable: false });
    Object.defineProperty(this, "_dataMap", { value: dataMap, enumerable: false });
  }
}

Object.defineProperty(LazyTextNode.prototype, "value", {
  get(this: LazyTextNode) {
    const val = this._reader.getTextValue(this._nodeId);
    Object.defineProperty(this, "value", {
      value: val,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    return val;
  },
  configurable: true,
  enumerable: true,
});

Object.defineProperty(LazyTextNode.prototype, "data", {
  get(this: LazyTextNode) {
    return this._dataMap.get(this._nodeId);
  },
  set(this: LazyTextNode, value: Record<string, unknown>) {
    this._dataMap.set(this._nodeId, value);
  },
  configurable: true,
  enumerable: true,
});

// Type name lookup for text-like nodes (hoisted to avoid per-node allocation)
const TEXT_TYPE_NAMES: Record<number, string> = {
  [HAST_TEXT]: "text",
  [HAST_COMMENT]: "comment",
  [HAST_RAW]: "raw",
  [HAST_MDX_FLOW_EXPRESSION]: "mdxFlowExpression",
  [HAST_MDX_TEXT_EXPRESSION]: "mdxTextExpression",
  [HAST_MDX_ESM]: "mdxjsEsm",
};

/** Fast materializer for the visitor — avoids per-node Object.defineProperty overhead. */
function materializeForVisitor(
  nodeType: number,
  nodeId: number,
  reader: HastReader,
  dataMap: DataMap,
): HastNode {
  switch (nodeType) {
    case HAST_ELEMENT:
      return new LazyElementNode(nodeId, reader, dataMap) as unknown as HastNode;
    case HAST_TEXT:
    case HAST_COMMENT:
    case HAST_RAW:
    case HAST_MDX_FLOW_EXPRESSION:
    case HAST_MDX_TEXT_EXPRESSION:
    case HAST_MDX_ESM:
      return new LazyTextNode(
        TEXT_TYPE_NAMES[nodeType]!,
        nodeId,
        reader,
        dataMap,
      ) as unknown as HastNode;
    default:
      // For root, mdxJsx*, doctype — fall back to full materializer
      return materializeHastNode(reader, nodeId, dataMap);
  }
}

// ---------------------------------------------------------------------------
// Selective walk helpers
// ---------------------------------------------------------------------------

interface ResolvedSubscription {
  nodeType: number;
  tagFilter: string[];
  visitFn: (node: HastNode, ctx: HastVisitorContext) => HastNode | void;
}

function isFilteredVisitor(v: unknown): v is HastFilteredVisitor {
  return typeof v === "object" && v !== null && "filter" in v && "visit" in v;
}

/** Node types that use filtered visitors (have tag/component names). */
const FILTERED_METHODS = new Set(["element", "mdxJsxFlowElement", "mdxJsxTextElement"]);

/**
 * Resolve subscriptions from a plugin instance.
 * Returns null if the plugin uses transformRoot (needs full buffer path).
 */
export function resolveSubscriptions(plugin: HastVisitorInstance): ResolvedSubscription[] | null {
  if (plugin.transformRoot) return null;

  const subs: ResolvedSubscription[] = [];

  for (const [methodName, nodeType] of Object.entries(METHOD_TO_TYPE)) {
    const value = plugin[methodName as keyof HastVisitorInstance];
    if (value === undefined) continue;

    if (FILTERED_METHODS.has(methodName)) {
      const items = Array.isArray(value) ? value : [value];
      for (const fv of items as HastFilteredVisitor[]) {
        subs.push({
          nodeType,
          tagFilter: fv.filter,
          visitFn: fv.visit as ResolvedSubscription["visitFn"],
        });
      }
    } else {
      // Bare function — empty filter matches all nodes of this type
      subs.push({ nodeType, tagFilter: [], visitFn: value as ResolvedSubscription["visitFn"] });
    }
  }

  return subs.length > 0 ? subs : null;
}

/** Reverse map: method name → node type number */
const METHOD_TO_TYPE: Record<string, number> = {
  element: HAST_ELEMENT,
  text: HAST_TEXT,
  comment: HAST_COMMENT,
  raw: HAST_RAW,
  doctype: 4, // HAST_DOCTYPE
  mdxJsxFlowElement: HAST_MDX_JSX_ELEMENT,
  mdxJsxTextElement: HAST_MDX_JSX_TEXT_ELEMENT,
  mdxFlowExpression: HAST_MDX_FLOW_EXPRESSION,
  mdxTextExpression: HAST_MDX_TEXT_EXPRESSION,
  mdxjsEsm: HAST_MDX_ESM,
};

/**
 * Selective walk path: Rust walks the tree, only sends matched nodes to JS.
 * Used when all plugin subscriptions have filters.
 */
const textDecoder = new TextDecoder("utf-8");

/** Read a matched element node from the binary data section into a HastNode. */
function readElementFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  resolver?: LazyChildResolver,
): HastNode {
  let pos = offset;

  // tagName
  const tagLen = view.getUint16(pos, true);
  pos += 2;
  const tagName = textDecoder.decode(buf.subarray(pos, pos + tagLen));
  pos += tagLen;

  // properties
  const propCount = view.getUint16(pos, true);
  pos += 2;
  const properties: Record<string, string | boolean | string[]> = {};
  for (let i = 0; i < propCount; i++) {
    const nameLen = view.getUint16(pos, true);
    pos += 2;
    const name = textDecoder.decode(buf.subarray(pos, pos + nameLen));
    pos += nameLen;
    const kind = buf[pos]!;
    pos += 1;
    const valLen = view.getUint16(pos, true);
    pos += 2;
    const valStr = textDecoder.decode(buf.subarray(pos, pos + valLen));
    pos += valLen;
    switch (kind) {
      case 0: // PROP_STRING
        properties[name] = valStr;
        break;
      case 1: // PROP_BOOL_TRUE
        properties[name] = true;
        break;
      case 2: // PROP_BOOL_FALSE
        // skip false booleans (matches HastReader behavior)
        break;
      case 3: // PROP_SPACE_SEP
        properties[name] = valStr.split(" ").filter((s) => s.length > 0);
        break;
    }
  }

  // Child IDs
  const childCount = view.getUint16(pos, true);
  pos += 2;
  const childIds: number[] = [];
  for (let i = 0; i < childCount; i++) {
    childIds.push(view.getUint32(pos, true));
    pos += 4;
  }

  const node = { type: "element" as const, tagName, properties } as unknown as HastNode &
    Record<string, unknown>;
  if (resolver) {
    makeLazyChildren(node, childIds, resolver);
  } else {
    (node as Record<string, unknown>).children = childIds.map((id) => ({
      _nodeId: id,
      type: "__child_ref__",
    }));
  }
  Object.defineProperty(node, "_nodeId", {
    value: nodeId,
    writable: false,
    configurable: true,
    enumerable: false,
  });
  return node;
}

/** Read a text/comment/raw node from the binary data section. */
const TEXT_NODE_TYPES: Record<number, string> = { 2: "text", 3: "comment", 5: "raw" };

function readTextFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
): HastNode {
  const valLen = view.getUint32(offset, true);
  const value = textDecoder.decode(buf.subarray(offset + 4, offset + 4 + valLen));
  const node = { type: TEXT_NODE_TYPES[nodeType]!, value } as unknown as HastNode;
  Object.defineProperty(node, "_nodeId", {
    value: nodeId,
    writable: false,
    configurable: true,
    enumerable: false,
  });
  return node;
}

/** Read an MDX JSX element from the binary data section. */
function readMdxJsxFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver?: LazyChildResolver,
): HastNode {
  let pos = offset;

  // Name
  const nameLen = view.getUint16(pos, true);
  pos += 2;
  const name = nameLen > 0 ? textDecoder.decode(buf.subarray(pos, pos + nameLen)) : null;
  pos += nameLen;

  // Attributes: [kind: u8][nameLen: u16][name][valLen: u16][val]
  const attrCount = view.getUint16(pos, true);
  pos += 2;
  const attributes: { type: string; name?: string; value: unknown }[] = [];
  for (let i = 0; i < attrCount; i++) {
    const kind = buf[pos]!;
    pos += 1;
    const attrNameLen = view.getUint16(pos, true);
    pos += 2;
    const attrName = textDecoder.decode(buf.subarray(pos, pos + attrNameLen));
    pos += attrNameLen;
    const attrValLen = view.getUint16(pos, true);
    pos += 2;
    const attrVal = textDecoder.decode(buf.subarray(pos, pos + attrValLen));
    pos += attrValLen;

    switch (kind) {
      case 0: // BooleanProp
        attributes.push({ type: "mdxJsxAttribute", name: attrName, value: null });
        break;
      case 1: // LiteralProp
        attributes.push({ type: "mdxJsxAttribute", name: attrName, value: attrVal });
        break;
      case 2: // ExpressionProp
        attributes.push({
          type: "mdxJsxAttribute",
          name: attrName,
          value: { type: "mdxJsxAttributeValueExpression", value: attrVal },
        });
        break;
      case 3: // Spread
        attributes.push({ type: "mdxJsxExpressionAttribute", value: attrVal });
        break;
    }
  }

  // Child IDs
  const childCount = view.getUint16(pos, true);
  pos += 2;
  const childIds: number[] = [];
  for (let i = 0; i < childCount; i++) {
    childIds.push(view.getUint32(pos, true));
    pos += 4;
  }

  const typeName = nodeType === HAST_MDX_JSX_ELEMENT ? "mdxJsxFlowElement" : "mdxJsxTextElement";
  const node = { type: typeName, name, attributes } as unknown as HastNode &
    Record<string, unknown>;
  if (resolver) {
    makeLazyChildren(node, childIds, resolver);
  } else {
    (node as Record<string, unknown>).children = childIds.map((id) => ({
      _nodeId: id,
      type: "__child_ref__",
    }));
  }
  Object.defineProperty(node, "_nodeId", {
    value: nodeId,
    writable: false,
    configurable: true,
    enumerable: false,
  });
  return node;
}

function readMatchedNode(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver?: LazyChildResolver,
): HastNode {
  if (nodeType === HAST_ELEMENT) {
    return readElementFromBinary(view, buf, offset, nodeId, resolver);
  } else if (nodeType === HAST_TEXT || nodeType === HAST_COMMENT || nodeType === HAST_RAW) {
    return readTextFromBinary(view, buf, offset, nodeId, nodeType);
  } else if (nodeType === HAST_MDX_JSX_ELEMENT || nodeType === HAST_MDX_JSX_TEXT_ELEMENT) {
    return readMdxJsxFromBinary(view, buf, offset, nodeId, nodeType, resolver);
  }
  // Fallback: minimal node
  return { type: `unknown(${nodeType})`, _nodeId: nodeId } as unknown as HastNode;
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/**
 * Lazy child materializer — serializes the handle's buffer once when first
 * child is accessed, then materializes children from it via HastReader.
 */
class LazyChildResolver {
  #handle: HastHandle;
  #reader: HastReader | null = null;
  #dataMap: DataMap | null = null;

  constructor(handle: HastHandle) {
    this.#handle = handle;
  }

  #ensure(): { reader: HastReader; dataMap: DataMap } {
    if (!this.#reader) {
      this.#reader = new HastReader(serializeHandle(this.#handle));
      this.#dataMap = new DataMap();
    }
    return { reader: this.#reader, dataMap: this.#dataMap! };
  }

  materializeChildren(childIds: number[]): HastNode[] {
    const { reader, dataMap } = this.#ensure();
    return childIds.map((id) => materializeHastNode(reader, id, dataMap));
  }
}

/** Create a lazy `children` property backed by the handle. */
function makeLazyChildren(
  node: Record<string, unknown>,
  childIds: number[],
  resolver: LazyChildResolver,
): void {
  Object.defineProperty(node, "children", {
    get() {
      const children = resolver.materializeChildren(childIds);
      Object.defineProperty(this, "children", {
        value: children,
        writable: true,
        enumerable: true,
        configurable: true,
      });
      return children;
    },
    configurable: true,
    enumerable: true,
  });
}

/** Dispatch matched nodes from a binary match buffer to visitor functions. */
function dispatchMatches(
  matchBuf: Uint8Array,
  subs: ResolvedSubscription[],
  ctx: HastVisitorContextImpl,
  returnBuffer: CommandBuffer,
  resolver?: LazyChildResolver,
): void {
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = matchView.getUint32(0, true);

  for (let i = 0; i < matchCount; i++) {
    const indexBase = 4 + i * 12;
    const nodeId = matchView.getUint32(indexBase, true);
    const subIndex = matchBuf[indexBase + 4]!;
    const dataOffset = matchView.getUint32(indexBase + 6, true);

    const sub = subs[subIndex]!;
    const node = readMatchedNode(matchView, matchBuf, dataOffset, nodeId, sub.nodeType, resolver);
    const result = sub.visitFn(node, ctx);
    if (result != null) {
      returnBuffer.replaceRawJson(nodeId, JSON.stringify(markHast(result)));
    }
  }
}

/** Merge return-value + context command buffers and release internals. */
function mergeAndReset(
  returnBuffer: CommandBuffer,
  ctx: HastVisitorContextImpl,
): { merged: Uint8Array; hasMutations: boolean } {
  const ctxCmdBuf = ctx.getCommandBuffer();
  const ctxBuf = ctxCmdBuf.getBuffer();
  const retBuf = returnBuffer.getBuffer();
  const totalLen = retBuf.length + ctxBuf.length;

  let merged: Uint8Array;
  if (totalLen === 0) {
    merged = new Uint8Array(0);
  } else {
    merged = new Uint8Array(totalLen);
    merged.set(retBuf, 0);
    merged.set(ctxBuf, retBuf.length);
  }

  returnBuffer.reset();
  ctxCmdBuf.reset();
  return { merged, hasMutations: totalLen > 0 };
}

// ---------------------------------------------------------------------------
// Handle-based visitor (primary path)
// ---------------------------------------------------------------------------

/**
 * Walk a handle's arena in Rust, dispatch matched nodes to JS visitor functions,
 * and apply mutations back to the handle. No arena buffers cross NAPI.
 */
export function visitHastHandle(
  handle: HastHandle,
  plugin: HastVisitorInstance,
  subs: ResolvedSubscription[],
): void {
  const ctx = new HastVisitorContextImpl();
  const returnBuffer = new CommandBuffer();

  plugin.before?.(ctx);

  const resolver = new LazyChildResolver(handle);
  const rustSubs = subs.map((s) => ({ nodeType: s.nodeType, tagFilter: s.tagFilter }));
  dispatchMatches(walkHandle(handle, rustSubs), subs, ctx, returnBuffer, resolver);

  plugin.after?.(ctx);

  const { merged, hasMutations } = mergeAndReset(returnBuffer, ctx);
  if (hasMutations) {
    applyCommandsToHandle(handle, merged);
  }
}

// ---------------------------------------------------------------------------
// Buffer-based visitor (fallback for transformRoot / bare function plugins)
// ---------------------------------------------------------------------------

// Map from node_type number to visitor method name
const TYPE_TO_METHOD: Record<number, keyof HastVisitorInstance> = {
  [HAST_ROOT]: "transformRoot",
  [HAST_ELEMENT]: "element",
  [HAST_TEXT]: "text",
  [HAST_COMMENT]: "comment",
  [HAST_RAW]: "raw",
  [HAST_MDX_JSX_ELEMENT]: "mdxJsxFlowElement",
  [HAST_MDX_JSX_TEXT_ELEMENT]: "mdxJsxTextElement",
  [HAST_MDX_FLOW_EXPRESSION]: "mdxFlowExpression",
  [HAST_MDX_TEXT_EXPRESSION]: "mdxTextExpression",
  [HAST_MDX_ESM]: "mdxjsEsm",
};

/**
 * Buffer fallback: walk a HAST binary buffer in JS and dispatch to visitor methods.
 * Used for transformRoot plugins and bare-function plugins that may return replacement nodes.
 */
export function visitHast(
  reader: HastReader,
  plugin: HastVisitorInstance,
  dataMap: DataMap,
): HastVisitResult {
  const ctx = new HastVisitorContextImpl();
  const returnBuffer = new CommandBuffer();

  plugin.before?.(ctx);

  if (typeof plugin.transformRoot === "function") {
    const root = materializeHastNode(reader, 0, dataMap) as Root;
    const result = plugin.transformRoot(root, ctx);
    if (result != null) {
      returnBuffer.replaceRawJson(0, JSON.stringify(markHast(result)));
    }
  } else {
    const stack: number[] = [0];
    while (stack.length > 0) {
      const nodeId = stack.pop()!;
      const nodeType = reader.getNodeType(nodeId);
      const methodName = TYPE_TO_METHOD[nodeType];

      if (methodName && methodName !== "transformRoot") {
        const entry = plugin[methodName];
        if (entry !== undefined) {
          let visitFn: ((node: HastNode, ctx: HastVisitorContext) => HastNode | void) | undefined;
          if (typeof entry === "function") {
            visitFn = entry as (node: HastNode, ctx: HastVisitorContext) => HastNode | void;
          } else if (isFilteredVisitor(entry)) {
            visitFn = entry.visit as (node: HastNode, ctx: HastVisitorContext) => HastNode | void;
          }
          if (visitFn) {
            const node = materializeForVisitor(nodeType, nodeId, reader, dataMap);
            const result = visitFn.call(plugin, node, ctx);
            if (result != null) {
              returnBuffer.replaceRawJson(nodeId, JSON.stringify(markHast(result)));
            }
          }
        }
      }

      reader.pushChildIds(nodeId, stack);
    }
  }

  plugin.after?.(ctx);

  const { merged, hasMutations } = mergeAndReset(returnBuffer, ctx);
  return { commandBuffer: merged, diagnostics: ctx.getDiagnostics(), hasMutations };
}
