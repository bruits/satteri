import { materializeHastNode, type HastNode } from "./hast-materializer.js";
import type { HastNodeInternal, HastRaw, MdxJsxAttributeUnion } from "../types.js";
import type { Element, Text, Comment, Doctype } from "hast";
import type { MdxJsxFlowElementHast, MdxJsxTextElementHast } from "../mdx-types.js";
import type { MdxFlowExpressionHast, MdxTextExpressionHast } from "../mdx-types.js";
import type { MdxjsEsmHast } from "../mdx-types.js";
import {
  HastReader,
  HAST_ELEMENT,
  HAST_TEXT,
  HAST_COMMENT,
  HAST_RAW,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
  HAST_MDX_FLOW_EXPRESSION,
  HAST_MDX_TEXT_EXPRESSION,
  HAST_MDX_ESM,
} from "./hast-reader.js";
import { CommandBuffer } from "../command-buffer.js";
import {
  walkHandle,
  applyCommandsToHandle,
  serializeHandle,
  textContentHandle,
  getNodeData as napiGetNodeData,
  parseExpression as napiParseExpression,
  parseEsm as napiParseEsm,
} from "#binding";

type NapiParseFn = (source: string) => string | null;

// Opaque handle type from NAPI, the arena lives in Rust memory.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type HastHandle = any;

/** ESTree-compatible Program node returned by `parseExpression()`. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type EstreeProgram = Record<string, any>;

/** Maps HastNode objects to their arena node IDs without Object.defineProperty overhead. */
const nodeIdMap: WeakMap<object, number> = new WeakMap();

/** Attach `parseExpression()` to an MDX expression or ESM node. */
function attachParseExpression(node: HastNode, parseFn: NapiParseFn): void {
  Object.defineProperty(node, "parseExpression", {
    value(): EstreeProgram | null {
      const value = (this as { value?: string }).value;
      if (typeof value !== "string") return null;
      const json = parseFn(value);
      if (json == null) return null;
      return JSON.parse(json) as EstreeProgram;
    },
    writable: false,
    enumerable: false,
    configurable: true,
  });
}

export interface HastDiagnostic {
  message: string;
  nodeId?: number | undefined;
  severity: "error" | "warning" | "info";
}

export interface HastVisitorContext {
  readonly source: string;
  readonly filename: string;
  removeNode(node: Readonly<HastNode>): void;
  replaceNode(node: Readonly<HastNode>, newNode: HastNode): void;
  insertBefore(node: Readonly<HastNode>, newNode: HastNode): void;
  insertAfter(node: Readonly<HastNode>, newNode: HastNode): void;
  wrapNode(node: Readonly<HastNode>, parentNode: HastNode): void;
  prependChild(node: Readonly<HastNode>, childNode: HastNode): void;
  appendChild(node: Readonly<HastNode>, childNode: HastNode): void;
  setProperty(node: Readonly<HastNode>, key: string, value: unknown): void;
  /** Collect the concatenated text of all descendant text nodes (like DOM textContent). */
  textContent(node: Readonly<HastNode>): string;
  report(opts: {
    message: string;
    node?: Readonly<HastNode>;
    severity?: "error" | "warning" | "info";
  }): void;
  getDiagnostics(): HastDiagnostic[];
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
  if ("children" in node) {
    obj.children = (n.children as HastNode[]).map(markHast);
  }
  return obj;
}

function nid(node: HastNode): number {
  return nodeIdMap.get(node as object) ?? (node as HastNodeInternal)._nodeId;
}

class HastVisitorContextImpl implements HastVisitorContext {
  readonly #commandBuffer: CommandBuffer = new CommandBuffer();
  readonly #diagnostics: HastDiagnostic[] = [];
  /** Track accumulated node state for multiple setProperty calls on the same node. */
  readonly #pendingNodes: Map<number, HastNode> = new Map();
  readonly #handle: HastHandle;
  readonly source: string;
  readonly filename: string;

  constructor(handle: HastHandle, source: string, filename: string) {
    this.#handle = handle;
    this.source = source;
    this.filename = filename;
  }

  removeNode(node: HastNode): void {
    this.#commandBuffer.removeNode(nid(node));
  }

  replaceNode(node: HastNode, newNode: HastNode): void {
    const id = nid(node);
    this.#commandBuffer.replaceRawJson(id, JSON.stringify(markHast(newNode)));
    this.#pendingNodes.set(id, newNode);
  }

  insertBefore(node: HastNode, newNode: HastNode): void {
    this.#commandBuffer.insertBeforeRawJson(nid(node), JSON.stringify(markHast(newNode)));
  }

  insertAfter(node: HastNode, newNode: HastNode): void {
    this.#commandBuffer.insertAfterRawJson(nid(node), JSON.stringify(markHast(newNode)));
  }

  wrapNode(node: HastNode, parentNode: HastNode): void {
    this.#commandBuffer.wrapNodeRawJson(nid(node), JSON.stringify(markHast(parentNode)));
  }

  prependChild(node: HastNode, childNode: HastNode): void {
    this.#commandBuffer.prependChildRawJson(nid(node), JSON.stringify(markHast(childNode)));
  }

  appendChild(node: HastNode, childNode: HastNode): void {
    this.#commandBuffer.appendChildRawJson(nid(node), JSON.stringify(markHast(childNode)));
  }

  setProperty(node: HastNode, key: string, value: unknown): void {
    const id = nid(node);
    if (node.type === "element") {
      // Fast binary path, no materialization, no JSON serialization
      this.#commandBuffer.setProperty(id, key, value);
      return;
    }

    if (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") {
      // MDX JSX nodes use `attributes`, not `properties`, keep replaceNode path
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

    // Text-like nodes (text, comment, raw, expressions, esm), fast binary path.
    // Rust handles "value" setProperty directly on these types.
    this.#commandBuffer.setProperty(id, key, value);
  }

  textContent(node: HastNode): string {
    return textContentHandle(this.#handle, nid(node));
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
  visit(node: Readonly<N>, ctx: HastVisitorContext): HastNode | void | Promise<HastNode | void>;
}

type HastVisitorFn<N extends HastNode = HastNode> = (
  node: Readonly<N>,
  ctx: HastVisitorContext,
) => HastNode | void | Promise<HastNode | void>;

export interface HastVisitorInstance {
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
  mdxFlowExpression?: HastVisitorFn<
    MdxFlowExpressionHast & { parseExpression(): EstreeProgram | null }
  >;
  mdxTextExpression?: HastVisitorFn<
    MdxTextExpressionHast & { parseExpression(): EstreeProgram | null }
  >;
  mdxjsEsm?: HastVisitorFn<MdxjsEsmHast & { parseExpression(): EstreeProgram | null }>;
}

// Selective walk helpers

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

/** Resolve subscriptions from a plugin instance. */
export function resolveSubscriptions(plugin: HastVisitorInstance): ResolvedSubscription[] {
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
      // Bare function, empty filter matches all nodes of this type
      subs.push({ nodeType, tagFilter: [], visitFn: value as ResolvedSubscription["visitFn"] });
    }
  }

  return subs;
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
 */
const textDecoder = new TextDecoder("utf-8");

/** Decode properties from the walk buffer at the given position. */
function decodeProperties(
  view: DataView,
  buf: Uint8Array,
  pos: number,
): Record<string, string | number | boolean | string[]> {
  const propCount = view.getUint16(pos, true);
  pos += 2;
  const properties: Record<string, string | number | boolean | string[]> = {};
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
        break;
      case 3: // PROP_SPACE_SEP
        properties[name] = valStr.split(" ").filter((s) => s.length > 0);
        break;
      case 5: // PROP_INT
        properties[name] = Number(valStr);
        break;
    }
  }
  return properties;
}

/**
 * Walk-path element node: uses prototype getters instead of per-instance
 * Object.defineProperty. V8 optimises shared hidden classes far better,
 * this is ~16x faster for construction than the defineProperty approach.
 *
 * The buffer reference data is stored on private instance fields so the
 * prototype getter can decode lazily on first access.
 */
class WalkElement {
  readonly type = "element" as const;
  declare tagName: string;

  /** @internal */ _view!: DataView;
  /** @internal */ _buf!: Uint8Array;
  /** @internal */ _propsPos!: number;
  /** @internal */ _childIds!: number[];
  /** @internal */ _resolver!: LazyChildResolver;
  /** @internal */ _dataPos!: number;
  /** @internal */ _dataLen!: number;

  get properties(): Record<string, string | number | boolean | string[]> {
    const val = decodeProperties(this._view, this._buf, this._propsPos);
    Object.defineProperty(this, "properties", {
      value: val,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    return val;
  }

  get children(): HastNode[] {
    const val = this._resolver.materializeChildren(this._childIds);
    Object.defineProperty(this, "children", {
      value: val,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    return val;
  }

  get data(): Record<string, unknown> | undefined {
    if (this._dataPos < 0) return undefined;
    const val = JSON.parse(
      textDecoder.decode(this._buf.subarray(this._dataPos, this._dataPos + this._dataLen)),
    ) as Record<string, unknown>;
    Object.defineProperty(this, "data", {
      value: val,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    return val;
  }
}

/** Read a matched element node from the binary data section into a HastNode.
 *  Only tagName is decoded eagerly; properties, children, and data are lazy. */
function readElementFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  resolver: LazyChildResolver,
): HastNode {
  let pos = offset;

  // Eager: tagName (almost always accessed by visitors)
  const tagLen = view.getUint16(pos, true);
  pos += 2;
  const tagName = textDecoder.decode(buf.subarray(pos, pos + tagLen));
  pos += tagLen;

  // Pre-scan: find section byte offsets without decoding strings
  const propsPos = pos;
  const propCount = view.getUint16(pos, true);
  pos += 2;
  for (let i = 0; i < propCount; i++) {
    const nLen = view.getUint16(pos, true);
    pos += 2 + nLen + 1; // name + kind byte
    const vLen = view.getUint16(pos, true);
    pos += 2 + vLen; // value
  }

  const childCount = view.getUint16(pos, true);
  pos += 2;
  const childIdsPos = pos;
  pos += childCount * 4;

  const nodeDataLen = view.getUint32(pos, true);
  pos += 4;
  const nodeDataPos = nodeDataLen > 0 ? pos : -1;

  // Collect child IDs for lazy materialization
  const ids: number[] = [];
  for (let i = 0; i < childCount; i++) ids.push(view.getUint32(childIdsPos + i * 4, true));

  // Build node using class (prototype getters, no per-instance defineProperty)
  const node = new WalkElement();
  node.tagName = tagName;
  node._view = view;
  node._buf = buf;
  node._propsPos = propsPos;
  node._childIds = ids;
  node._resolver = resolver;
  node._dataPos = nodeDataPos;
  node._dataLen = nodeDataLen;
  nodeIdMap.set(node as object, nodeId);

  return node as unknown as HastNode;
}

/** Read a text/comment/raw/expression node from the binary data section. */
const TEXT_NODE_TYPES: Record<number, string> = {
  2: "text",
  3: "comment",
  5: "raw",
  [HAST_MDX_FLOW_EXPRESSION]: "mdxFlowExpression",
  [HAST_MDX_TEXT_EXPRESSION]: "mdxTextExpression",
  [HAST_MDX_ESM]: "mdxjsEsm",
};

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
  nodeIdMap.set(node as object, nodeId);
  if (nodeType === HAST_MDX_FLOW_EXPRESSION || nodeType === HAST_MDX_TEXT_EXPRESSION) {
    attachParseExpression(node, napiParseExpression);
  } else if (nodeType === HAST_MDX_ESM) {
    attachParseExpression(node, napiParseEsm);
  }
  return node;
}

/** Read an MDX JSX element from the binary data section. */
function readMdxJsxFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver: LazyChildResolver,
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
    const attrValLen = view.getUint32(pos, true);
    pos += 4;
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
  nodeIdMap.set(node as object, nodeId);
  makeLazyChildren(node, childIds, resolver);
  return node;
}

function readMatchedNode(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver: LazyChildResolver,
): HastNode {
  if (nodeType === HAST_ELEMENT) {
    return readElementFromBinary(view, buf, offset, nodeId, resolver);
  } else if (
    nodeType === HAST_TEXT ||
    nodeType === HAST_COMMENT ||
    nodeType === HAST_RAW ||
    nodeType === HAST_MDX_FLOW_EXPRESSION ||
    nodeType === HAST_MDX_TEXT_EXPRESSION ||
    nodeType === HAST_MDX_ESM
  ) {
    return readTextFromBinary(view, buf, offset, nodeId, nodeType);
  } else if (nodeType === HAST_MDX_JSX_ELEMENT || nodeType === HAST_MDX_JSX_TEXT_ELEMENT) {
    return readMdxJsxFromBinary(view, buf, offset, nodeId, nodeType, resolver);
  }
  // Fallback: minimal node
  const node = { type: `unknown(${nodeType})` } as unknown as HastNode;
  nodeIdMap.set(node as object, nodeId);
  return node;
}

// Shared helpers

/**
 * Lazy child materializer, serializes the handle's buffer once when first
 * child is accessed, then materializes children from it via HastReader.
 */
class LazyChildResolver {
  #handle: HastHandle;
  #reader: HastReader | null = null;

  constructor(handle: HastHandle) {
    this.#handle = handle;
  }

  #ensure(): HastReader {
    if (!this.#reader) {
      this.#reader = new HastReader(serializeHandle(this.#handle));
    }
    return this.#reader;
  }

  materializeChildren(childIds: number[]): HastNode[] {
    const reader = this.#ensure();
    const handle = this.#handle;
    return childIds.map((id) => {
      const node = materializeHastNode(reader, id);
      // Override data with a lazy getter backed by the Rust arena's node_data.
      Object.defineProperty(node, "data", {
        get() {
          const json = napiGetNodeData(handle, id);
          const val = json ? (JSON.parse(json) as Record<string, unknown>) : null;
          Object.defineProperty(this, "data", {
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
      return node;
    });
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

/** Handle a visitor result (sync).
 *  If the result is the same object as the input node, treat it as a no-op
 *  so that context mutations (e.g. setProperty) are not clobbered. */
function handleVisitResult(
  result: HastNode | void | Promise<HastNode | void>,
  nodeId: number,
  returnBuffer: CommandBuffer,
  deferred: { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[] | null,
  originalNode: HastNode,
): { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[] | null {
  if (result == null) return deferred;
  if (result === originalNode) return deferred;
  if (result instanceof Promise) {
    const list = deferred ?? [];
    list.push({ nodeId, promise: result, originalNode });
    return list;
  }
  returnBuffer.replaceRawJson(nodeId, JSON.stringify(markHast(result as HastNode)));
  return deferred;
}

/**
 * Dispatch matched nodes from a binary match buffer to visitor functions.
 * Returns null if all sync, or an array of deferred promises if any visitor was async.
 */
function dispatchMatches(
  matchBuf: Uint8Array,
  subs: ResolvedSubscription[],
  ctx: HastVisitorContextImpl,
  returnBuffer: CommandBuffer,
  resolver: LazyChildResolver,
): { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[] | null {
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = matchView.getUint32(0, true);
  let deferred:
    | { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[]
    | null = null;

  for (let i = 0; i < matchCount; i++) {
    const indexBase = 4 + i * 12;
    const nodeId = matchView.getUint32(indexBase, true);
    const subIndex = matchBuf[indexBase + 4]!;
    const dataOffset = matchView.getUint32(indexBase + 6, true);

    const sub = subs[subIndex]!;
    const node = readMatchedNode(matchView, matchBuf, dataOffset, nodeId, sub.nodeType, resolver);
    const result = sub.visitFn(node, ctx);
    deferred = handleVisitResult(result, nodeId, returnBuffer, deferred, node);
  }

  return deferred;
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

// Handle-based visitor

/**
 * Walk a handle's arena in Rust, dispatch matched nodes to JS visitor functions,
 * and apply mutations back to the handle. No arena buffers cross NAPI.
 *
 * Returns void if all visitors are sync, or a Promise if any visitor is async.
 */
export function visitHastHandle(
  handle: HastHandle,
  plugin: HastVisitorInstance,
  subs: ResolvedSubscription[],
  source: string,
  filename: string,
): void | Promise<void> {
  const ctx = new HastVisitorContextImpl(handle, source, filename);
  const returnBuffer = new CommandBuffer();
  const resolver = new LazyChildResolver(handle);
  const rustSubs = subs.map((s) => ({ nodeType: s.nodeType, tagFilter: s.tagFilter }));
  const deferred = dispatchMatches(walkHandle(handle, rustSubs), subs, ctx, returnBuffer, resolver);

  if (deferred) {
    return Promise.all(
      deferred.map((d) =>
        d.promise.then((result) => ({ nodeId: d.nodeId, result, originalNode: d.originalNode })),
      ),
    ).then((results) => {
      for (const { nodeId, result, originalNode } of results) {
        if (result != null && result !== originalNode) {
          returnBuffer.replaceRawJson(nodeId, JSON.stringify(markHast(result)));
        }
      }
      applyMutations(handle, returnBuffer, ctx);
    });
  }

  applyMutations(handle, returnBuffer, ctx);
}

function applyMutations(
  handle: HastHandle,
  returnBuffer: CommandBuffer,
  ctx: HastVisitorContextImpl,
): void {
  const { merged, hasMutations } = mergeAndReset(returnBuffer, ctx);
  if (hasMutations) {
    applyCommandsToHandle(handle, merged);
  }
}
