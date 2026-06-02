import { materializeHastNode, type HastNode } from "./hast-materializer.js";
import type { HastNodeInternal, HastRaw, MdxJsxAttributeUnion, Position } from "../types.js";
import type { Element, Text, Comment, Doctype } from "hast";
import type { Program } from "estree-jsx";
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
export type EstreeProgram = Program;

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
  /**
   * The URL of the document being processed (the compile `fileURL` option),
   * or `undefined` when none was given. Use `fileURLToPath(ctx.fileURL)` for a
   * decoded filesystem path.
   */
  readonly fileURL: URL | undefined;
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

/**
 * Serialize a HastNode for the command buffer. Marks each node `_hast: true`,
 * and — except at the root, which is the new replacement content — emits a
 * reused (materialized) node as a `{ _ref: id }` placeholder so the rebuild
 * splices the original in place (preserving its id, applying any pending patch
 * on it) instead of rebuilding it fresh.
 */
function markHast(node: HastNode): Record<string, unknown> {
  return markHastNode(node, true);
}

function markHastNode(node: HastNode, isRoot: boolean): Record<string, unknown> {
  if (!isRoot) {
    const id = nodeIdMap.get(node) ?? (node as HastNodeInternal)._nodeId;
    if (typeof id === "number") return { _ref: id };
  }
  const obj: Record<string, unknown> = { _hast: true, type: node.type };
  if ("tagName" in node) obj.tagName = node.tagName;
  if ("properties" in node) obj.properties = node.properties;
  if ("value" in node) obj.value = node.value;
  if ("name" in node) obj.name = node.name;
  if ("attributes" in node) obj.attributes = node.attributes;
  if ("data" in node && node.data != null) obj.data = node.data;
  if ("children" in node) {
    obj.children = node.children.map((c) => markHastNode(c, false));
  }
  return obj;
}

function nid(node: HastNode): number {
  return nodeIdMap.get(node) ?? (node as HastNodeInternal)._nodeId;
}

class HastVisitorContextImpl implements HastVisitorContext {
  readonly #commandBuffer: CommandBuffer = new CommandBuffer();
  readonly #diagnostics: HastDiagnostic[] = [];
  /** Track accumulated node state for multiple setProperty calls on the same node. */
  readonly #pendingNodes: Map<number, HastNode> = new Map();
  readonly #handle: HastHandle;
  readonly #getSource: () => string;
  readonly fileURL: URL | undefined;

  constructor(handle: HastHandle, getSource: () => string, fileURL: URL | undefined) {
    this.#handle = handle;
    this.#getSource = getSource;
    this.fileURL = fileURL;
  }

  get source(): string {
    const value = this.#getSource();
    Object.defineProperty(this, "source", { value, writable: false, enumerable: true });
    return value;
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
    if (key === "data") {
      this.#commandBuffer.setProperty(id, key, value != null ? JSON.stringify(value) : null);
      return;
    }
    if (node.type === "element") {
      // Fast binary path, no materialization, no JSON serialization
      this.#commandBuffer.setProperty(id, key, value);
      return;
    }

    if (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") {
      // MDX JSX nodes use `attributes`, not `properties`, keep replaceNode path
      const pending = this.#pendingNodes.get(id);
      const current = (pending ?? node) as MdxJsxFlowElementHast | MdxJsxTextElementHast;
      const updated = { ...current };
      const attrs: MdxJsxAttributeUnion[] = [...(updated.attributes ?? [])];
      const idx = attrs.findIndex((a) => a.type === "mdxJsxAttribute" && a.name === key);
      if (idx !== -1) attrs.splice(idx, 1);
      const attrValue =
        value === true || value === null || value === undefined
          ? null
          : typeof value === "string"
            ? value
            : String(value);
      attrs.push({ type: "mdxJsxAttribute", name: key, value: attrValue });
      updated.attributes = attrs;
      this.replaceNode(node, updated);
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

/** Decode the 24-byte position block written by `serialize_hast_node_inline`. */
function readPositionPrefix(view: DataView, offset: number): Position | undefined {
  const startOffset = view.getUint32(offset, true);
  const endOffset = view.getUint32(offset + 4, true);
  const startLine = view.getUint32(offset + 8, true);
  const startColumn = view.getUint32(offset + 12, true);
  const endLine = view.getUint32(offset + 16, true);
  const endColumn = view.getUint32(offset + 20, true);
  if (startLine === 0 && startOffset === 0) return undefined;
  return {
    start: { offset: startOffset, line: startLine, column: startColumn },
    end: { offset: endOffset, line: endLine, column: endColumn },
  };
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
  declare position?: Position | undefined;
  declare data?: Record<string, unknown> | undefined;

  /** @internal */ _view!: DataView;
  /** @internal */ _buf!: Uint8Array;
  /** @internal */ _propsPos!: number;
  /** @internal */ _childIds!: number[];
  /** @internal */ _resolver!: LazyChildResolver;

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
}

/** Read the tail of a matched element node (tag + properties).
 *  Common prelude (data/position/children) is already consumed by `readMatchedNode`. */
function readElementFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  resolver: LazyChildResolver,
  position: Position | undefined,
  childIds: number[],
  data: Record<string, unknown> | null,
): HastNode {
  let pos = offset;

  // Eager: tagName (almost always accessed by visitors)
  const tagLen = view.getUint16(pos, true);
  pos += 2;
  const tagName = textDecoder.decode(buf.subarray(pos, pos + tagLen));
  pos += tagLen;

  const propsPos = pos;

  // Build node using class (prototype getters, no per-instance defineProperty)
  const node = new WalkElement();
  node.tagName = tagName;
  if (position !== undefined) node.position = position;
  if (data !== null) node.data = data;
  node._view = view;
  node._buf = buf;
  node._propsPos = propsPos;
  node._childIds = childIds;
  node._resolver = resolver;
  nodeIdMap.set(node, nodeId);

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
  position: Position | undefined,
  data: Record<string, unknown> | null,
): HastNode {
  const valLen = view.getUint32(offset, true);
  const value = textDecoder.decode(buf.subarray(offset + 4, offset + 4 + valLen));
  const base: Record<string, unknown> = { type: TEXT_NODE_TYPES[nodeType]!, value };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  const node = base as unknown as HastNode;
  nodeIdMap.set(node, nodeId);
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
  position: Position | undefined,
  childIds: number[],
  data: Record<string, unknown> | null,
): HastNode {
  let pos = offset;

  // Name
  const nameLen = view.getUint16(pos, true);
  pos += 2;
  const name = nameLen > 0 ? textDecoder.decode(buf.subarray(pos, pos + nameLen)) : null;
  pos += nameLen;

  // Attributes: [kind: u8][nameLen: u16][name][valLen: u32][val]
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

  const typeName = nodeType === HAST_MDX_JSX_ELEMENT ? "mdxJsxFlowElement" : "mdxJsxTextElement";
  const base: Record<string, unknown> = { type: typeName, name, attributes };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  nodeIdMap.set(base, nodeId);
  makeLazyChildren(base, childIds, resolver);
  return base as unknown as HastNode;
}

function readMatchedNode(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver: LazyChildResolver,
): HastNode {
  let pos = offset;

  // Shared prelude (matches serialize_hast_node_inline / serialize_mdast_node_inline):
  //   [data_len: u32][data_bytes][position: 24B][child_count: u16][child_ids: N×u32]

  // Data (JSON), eagerly parsed
  const dataLen = view.getUint32(pos, true);
  pos += 4;
  let data: Record<string, unknown> | null = null;
  if (dataLen > 0) {
    const jsonStr = textDecoder.decode(buf.subarray(pos, pos + dataLen));
    try {
      data = JSON.parse(jsonStr) as Record<string, unknown>;
    } catch {
      /* ignore malformed JSON */
    }
    pos += dataLen;
  }

  const position = readPositionPrefix(view, pos);
  pos += 24;

  const childCount = view.getUint16(pos, true);
  pos += 2;
  const childIds: number[] = [];
  for (let i = 0; i < childCount; i++) {
    childIds.push(view.getUint32(pos, true));
    pos += 4;
  }

  // Dispatch to type-specific tail (pos now sits at the type-specific section)
  if (nodeType === HAST_ELEMENT) {
    return readElementFromBinary(view, buf, pos, nodeId, resolver, position, childIds, data);
  } else if (
    nodeType === HAST_TEXT ||
    nodeType === HAST_COMMENT ||
    nodeType === HAST_RAW ||
    nodeType === HAST_MDX_FLOW_EXPRESSION ||
    nodeType === HAST_MDX_TEXT_EXPRESSION ||
    nodeType === HAST_MDX_ESM
  ) {
    return readTextFromBinary(view, buf, pos, nodeId, nodeType, position, data);
  } else if (nodeType === HAST_MDX_JSX_ELEMENT || nodeType === HAST_MDX_JSX_TEXT_ELEMENT) {
    return readMdxJsxFromBinary(
      view,
      buf,
      pos,
      nodeId,
      nodeType,
      resolver,
      position,
      childIds,
      data,
    );
  }
  // Fallback: minimal node carrying whatever prelude data we found
  const base: Record<string, unknown> = { type: `unknown(${nodeType})` };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  const node = base as unknown as HastNode;
  nodeIdMap.set(node, nodeId);
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
    return childIds.map((id) => {
      const node = materializeHastNode(reader, id);
      this.attachLazyData(node, id);
      return node;
    });
  }

  /** Attach a lazy `data` getter backed by the Rust arena's node_data. */
  attachLazyData(node: object, nodeId: number): void {
    const handle = this.#handle;
    Object.defineProperty(node, "data", {
      get() {
        const json = napiGetNodeData(handle, nodeId);
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
  }
}

/** Create a lazy `children` property backed by the handle. */
function makeLazyChildren(node: object, childIds: number[], resolver: LazyChildResolver): void {
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
  returnBuffer.replaceRawJson(nodeId, JSON.stringify(markHast(result)));
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
  source: string | (() => string),
  fileURL: URL | undefined,
): void | Promise<void> {
  const getSource = typeof source === "function" ? source : () => source;
  const ctx = new HastVisitorContextImpl(handle, getSource, fileURL);
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
