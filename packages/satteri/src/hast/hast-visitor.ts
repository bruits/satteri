import { applyCommandsToHandle, nodeLabels, textContentHandle, walkHandle } from "#binding";
import type { Program } from "estree-jsx";
import type {
  Comment,
  Doctype,
  Element,
  Parents as HastParents,
  Root as HastRoot,
  Text,
} from "hast";
import {
  acquireCommandBuffer,
  CommandBuffer,
  releaseCommandBuffer,
  STRUCTURAL_CMD,
  type StructuralOp,
} from "../command-buffer.js";
import { CMD_SET_CHILDREN } from "../generated/wire-constants.js";
import type {
  MdxFlowExpressionHast,
  MdxjsEsmHast,
  MdxJsxFlowElementHast,
  MdxJsxTextElementHast,
  MdxTextExpressionHast,
} from "../mdx-types.js";
import {
  emitMdxAttr,
  mdxAttributesJson,
  OF_EXPLICIT,
  OF_NAME,
  OF_TAGNAME,
  OF_VALUE,
  OpWriter,
  PROP_BOOL_FALSE,
  PROP_BOOL_TRUE,
  PROP_INT,
  PROP_TOKEN_LIST,
  PROP_STRING,
  PropRows,
  type PropSink,
} from "../op-stream.js";
import type { Data, HastRaw, SourceFormat } from "../types.js";
import { HAST_OPSTREAM_TYPES, NAME_TO_TYPE, VISITOR_KEYS } from "./generated/node-types.js";
import { type HastNode } from "./hast-materializer.js";
import { encodeTokenList } from "./element-props.js";
import {
  HAST_ELEMENT,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
  HAST_ROOT,
  HastReader,
} from "./hast-reader.js";
import { getNodeId, HastLazyChildResolver, readMatchedNode, type WalkWire } from "./hast-walk.js";

import type { HastHandle } from "../handles.js";
import { LazyChildResolver, markHandleMutated } from "../lazy-child-resolver.js";
import {
  asArray,
  withPluginErrors,
  takeCommands,
  FOREIGN_REF,
  foreignContentError,
  makeRequireNid,
  requireRootReplacement,
  RemovalTracker,
  existingWrapperError,
  requireContent,
  requireVisitorResult,
  requireWrapper,
  ROOT_NODE_ID,
  rootReplacementError,
  selfInsertError,
  type NodeRefs,
  type PluginOptions,
  type SettableFieldKey,
} from "../visitor-shared.js";

import {
  dataJson,
  describeValue,
  nodeOfType,
  requireSettableField,
  requireSiblingAnchor,
  requireTagName,
  requireHastPropertyValue,
  requireMdxAttributeValue,
  unbuildableNodeError,
  usageError,
} from "../usage-errors.js";

export type { HastHandle };

/** ESTree-compatible Program node returned by `parseExpression()`. */
export type EstreeProgram = Program;

export interface HastDiagnostic {
  message: string;
  nodeId?: number | undefined;
  position?: HastNode["position"] | undefined;
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
  /**
   * Document-level data bag, shared across every plugin in the compile and
   * across the mdast→hast phase boundary. Mutate keys directly
   * (`ctx.data.foo = x`); the bag itself isn't reassignable. Values are kept
   * on the JS side, so any value is allowed, including functions and class
   * instances. Returned to the caller as `result.data`.
   */
  readonly data: Data;
  /**
   * The source format this compile is processing: `"markdown"` for a plain
   * Markdown compile, `"mdx"` for an MDX one. Lets a plugin shared between both
   * pipelines branch on which it is handling.
   */
  readonly sourceFormat: SourceFormat;
  removeNode(node: Readonly<HastNode>): void;
  /**
   * Swap `node` for one node, or for an array of nodes placed in order at its
   * position. An empty array drops the node, the same as `removeNode`.
   * The document root takes a `root` and nothing else: the one place a `root`
   * is accepted as content.
   */
  replaceNode(node: Readonly<HastNode>, newNode: HastContent | HastContent[]): void;
  insertBefore(node: Readonly<HastNode>, newNode: HastContent | HastContent[]): void;
  insertAfter(node: Readonly<HastNode>, newNode: HastContent | HastContent[]): void;
  /**
   * Wrap `node` in `parentNode`, making it `parentNode`'s first child. Any
   * children `parentNode` declares are kept after it, so a `div` with an anchor
   * child wraps a heading as `div > [heading, anchor]`. `parentNode` is an
   * element, an MDX JSX element, or `{ raw }` HTML parsing to exactly one
   * element, never a void element, whose children would not render.
   */
  wrapNode(
    node: Readonly<HastNode>,
    parentNode: HastParentContent | RawHastContent | RawHtmlHastContent,
  ): void;
  prependChild(node: Readonly<HastNode>, childNode: HastContent | HastContent[]): void;
  appendChild(node: Readonly<HastNode>, childNode: HastContent | HastContent[]): void;
  /** Insert one node or an array at `index`; clamps (`0` or less prepends, past the end appends). */
  insertChildAt(
    node: Readonly<HastNode>,
    index: number,
    childNode: HastContent | HastContent[],
  ): void;
  /** Remove the `index`-th child of `node`; a no-op when there is no such child. */
  removeChildAt(node: Readonly<HastNode>, index: number): void;
  /** Replace a supported field on the node, including scalars and structured fields. */
  setField<N extends HastNode, K extends SettableFieldKey<N>>(
    node: Readonly<N>,
    key: K,
    value: Exclude<N[K], undefined>,
  ): void;
  /** `children` is structural and every parent accepts it, including node-type unions. */
  setField(node: Readonly<HastNode>, key: "children", value: readonly HastContent[]): void;
  /** `data` is an open per-node bag serialized to JSON. `null` clears it. */
  setField(node: Readonly<HastNode>, key: "data", value: Record<string, unknown> | null): void;
  /** Set one entry in an MDX JSX element's `attributes`. */
  setAttribute(node: Readonly<HastNode>, name: string, value: unknown): void;
  /**
   * Set one entry in a hast element's `properties`.
   *
   * Using this for node fields or MDX JSX attributes is deprecated; use
   * `setField` or `setAttribute` respectively.
   */
  setProperty(node: Readonly<HastNode>, key: string, value: unknown): void;
  /** Collect the concatenated text of all descendant text nodes (like DOM textContent). */
  textContent(node: Readonly<HastNode>): string;
  /**
   * The parent of a node, or `undefined` at the root. Within a pass the same
   * parent is always the same object, so visitors on sibling nodes can dedupe
   * by identity.
   */
  parent<N extends Exclude<HastNode, HastRoot>>(node: Readonly<N>): Readonly<HastParents>;
  parent(node: Readonly<HastNode>): Readonly<HastParents> | undefined;
  /**
   * Index of `node` within its parent's children, or `undefined` at the root.
   * Use this rather than `parent.children.indexOf(node)`, which won't find it.
   */
  indexOf(node: Readonly<HastNode>): number | undefined;
  report(opts: {
    message: string;
    node?: Readonly<HastNode>;
    severity?: "error" | "warning" | "info";
  }): void;
  getDiagnostics(): HastDiagnostic[];
}

const requireNid = makeRequireNid(getNodeId);

/** New content for a HAST structural mutation. Unlike [`MdastContent`], HAST has
 *  a `raw` node type, so it needs no raw/rawHtml escape hatch. */
export type HastContent = HastNode;

/** A `wrapNode` wrapper: node types that can hold children. */
export type HastParentContent = Exclude<Extract<HastNode, { children: unknown[] }>, HastRoot>;

/** Raw `wrapNode` wrapper: the HTML is parsed at apply time (not call time)
 *  and must yield exactly one non-void element, which becomes the wrapper.
 *  `mdxExpressions` is accepted for parity with the MDAST phase and has no
 *  effect: braces in HTML text are always literal. */
export interface RawHastContent {
  raw: string;
  mdxExpressions?: boolean;
}

export interface RawHtmlHastContent {
  /** @deprecated Use the equivalent `{ raw }`. */
  rawHtml: string;
}

const HAST_PARENT_TYPES = ["element", "mdxJsxFlowElement", "mdxJsxTextElement"] as const;
const HAST_PARENT_TYPE_SET = new Set<string>(HAST_PARENT_TYPES);

/** Compile error if the allowlist and {@link HastParentContent} drift apart. */
type AssertNever<T extends never> = T;
type _EveryHastParentIsListed = AssertNever<
  Exclude<HastParentContent["type"], (typeof HAST_PARENT_TYPES)[number]>
>;
type _EveryListedTypeIsAParent = AssertNever<
  Exclude<(typeof HAST_PARENT_TYPES)[number], HastParentContent["type"]>
>;

/** A leaf wrapper would make the patch engine drop or displace the wrapped node. */
function assertHastWrapParent(parentNode: HastContent): void {
  const type = (parentNode as { type?: unknown }).type;
  if (typeof type === "string" && HAST_PARENT_TYPE_SET.has(type)) return;
  throw usageError(
    `wrapNode: "${String(type)}" nodes cannot hold children, so they cannot wrap a node. ` +
      'Wrap in an element instead, e.g. { type: "element", tagName: "div", properties: {}, children: [] } ' +
      'or { raw: "<div></div>" }.',
  );
}

function hastReusedId(node: unknown, refs: NodeRefs): number | undefined {
  if (node === null || typeof node !== "object") return undefined;
  const id = getNodeId(node as HastNode, refs);
  return id !== undefined && id !== FOREIGN_REF ? id : undefined;
}

function emitHastChildrenCommand(
  buffer: CommandBuffer,
  id: number,
  children: readonly unknown[],
  refs: NodeRefs,
): void {
  buffer.emitOpstreamCommand(CMD_SET_CHILDREN, id, () => {
    buffer.open(HAST_ROOT);
    for (const c of children) emitHastOp(buffer, c, false, refs);
    buffer.close();
  });
}

function emitHastTree(
  buffer: CommandBuffer,
  op: StructuralOp,
  id: number,
  node: HastNode,
  refs: NodeRefs,
  allowRootRef = false,
): void {
  buffer.emitOpstreamCommand(STRUCTURAL_CMD[op], id, () =>
    emitHastOp(buffer, node, true, refs, false, allowRootRef),
  );
}

/** Several nodes as one edit, root-wrapped so the engine splices them in at the target. */
function emitHastSequence(
  buffer: CommandBuffer,
  cmd: number,
  id: number,
  nodes: readonly HastContent[],
  refs: NodeRefs,
): void {
  buffer.emitOpstreamCommand(cmd, id, () => {
    buffer.open(HAST_ROOT);
    for (const n of nodes) emitHastOp(buffer, n, false, refs);
    buffer.close();
  });
}

// Root replacement needs a separate encoder because per-node encoding rejects root payloads.
function emitHastRootReplace(buffer: CommandBuffer, root: HastContent, refs: NodeRefs): void {
  buffer.emitOpstreamCommand(STRUCTURAL_CMD.replace, ROOT_NODE_ID, () =>
    emitHastRootOp(buffer, root as unknown as Record<string, unknown>, refs),
  );
}

/** A document is encoded standalone, so no node can be a ref into a tree. */
const NO_REFS: NodeRefs = new WeakMap();

const HAST_DOCTYPE = NAME_TO_TYPE.doctype;

/**
 * Encode a standalone HAST tree as an op-stream document: one `root` wrapping
 * `nodes`, which Rust replays into an arena of its own and serializes. The
 * bytes are handed to `use` because they are a view into a pooled buffer,
 * valid only until it is released.
 */
export function encodeHastDocument<T>(nodes: readonly HastNode[], use: (ops: Uint8Array) => T): T {
  const w = acquireCommandBuffer();
  try {
    w.open(HAST_ROOT);
    for (const node of nodes) {
      // Carries no fields, and patch content has nothing to attach it to.
      if (node.type === "doctype" && HAST_DOCTYPE !== undefined) {
        w.open(HAST_DOCTYPE);
        w.close();
        continue;
      }
      emitHastOp(w, node, false, NO_REFS, true);
    }
    w.close();
    return use(w.getBuffer());
  } finally {
    releaseCommandBuffer(w);
  }
}

function emitHastRootOp(w: OpWriter, n: Record<string, unknown>, refs: NodeRefs): void {
  w.open(HAST_ROOT);
  if (n.data != null) w.data(n.data);
  const children = n.children;
  if (Array.isArray(children)) {
    for (const c of children) emitHastOp(w, c, false, refs);
  }
  w.close();
}

function emitHastOp(
  w: OpWriter,
  node: unknown,
  isRoot: boolean,
  refs: NodeRefs,
  document = false,
  allowRootRef = false,
): void {
  if (node === null || typeof node !== "object" || Array.isArray(node)) {
    throw unbuildableNodeError(node, false);
  }
  const id = getNodeId(node as HastNode, refs);
  if (id === FOREIGN_REF && !document) throw foreignContentError();
  if ((!isRoot || allowRootRef) && id !== undefined && id !== FOREIGN_REF) {
    w.ref(id);
    return;
  }
  const n = node as Record<string, unknown>;
  const type = HAST_OPSTREAM_TYPES[n.type as string];
  if (type === undefined) {
    throw unbuildableNodeError(n, typeof n.type === "string" && NAME_TO_TYPE[n.type] !== undefined);
  }
  // Standalone HTML serialization does not need MDX support or JSON-serializable metadata.
  if (document && (n.type as string).startsWith("mdx")) return;
  w.open(type);
  // A standalone document is only serialized, and has no source for a position to point into.
  if (!document) w.position(n.position);
  if (type === HAST_ELEMENT) {
    requireTagName(n.tagName);
    w.str(OF_TAGNAME, n.tagName as string);
    const props = n.properties;
    if (props !== null && typeof props === "object") {
      for (const key in props as Record<string, unknown>) {
        emitHastProp(w, key, (props as Record<string, unknown>)[key]);
      }
    }
  } else if (type === HAST_MDX_JSX_ELEMENT || type === HAST_MDX_JSX_TEXT_ELEMENT) {
    const name =
      typeof n.name === "string" ? n.name : typeof n.tagName === "string" ? n.tagName : "";
    if (name !== "") w.str(OF_NAME, name);
    if (Array.isArray(n.attributes)) {
      for (const a of n.attributes) emitMdxAttr(w, a as Record<string, unknown>);
    }
    if ((n.data as Record<string, unknown> | null | undefined)?._mdxExplicitJsx === true) {
      w.bool(OF_EXPLICIT, true);
    }
  } else {
    w.str(OF_VALUE, typeof n.value === "string" ? n.value : "");
  }
  if (!document && n.data != null) w.data(n.data);
  const children = n.children;
  if (Array.isArray(children)) {
    for (const c of children) emitHastOp(w, c, false, refs, document);
  }
  w.close();
}

function propertiesJson(value: unknown): string {
  if (value === null || value === undefined) return "[]";
  if (typeof value !== "object" || Array.isArray(value)) {
    throw usageError(
      `an element's "properties" is an object, not ${describeValue(value)}`,
      TypeError,
    );
  }
  const rows = new PropRows();
  for (const name in value as Record<string, unknown>) {
    emitHastProp(rows, name, (value as Record<string, unknown>)[name]);
  }
  return JSON.stringify(rows.rows);
}

function requireChildren(children: unknown): void {
  if (!Array.isArray(children)) {
    throw usageError(
      `setField: "children" takes an array of nodes, not ${describeValue(children)}`,
      TypeError,
    );
  }
  requireContent("setField", children);
}

function emitHastProp(w: PropSink, name: string, value: unknown): void {
  requireHastPropertyValue(name, value);
  if (value === true) w.prop(name, PROP_BOOL_TRUE, "");
  else if (value === false) w.prop(name, PROP_BOOL_FALSE, "");
  else if (typeof value === "string") w.prop(name, PROP_STRING, value);
  // A NaN property is dropped, not rendered as `"NaN"`, matching hast.
  else if (typeof value === "number") {
    if (!Number.isNaN(value)) w.prop(name, PROP_INT, String(value));
  } else if (Array.isArray(value)) w.prop(name, PROP_TOKEN_LIST, encodeTokenList(value));
}

class HastVisitorContextImpl implements HastVisitorContext {
  readonly #commandBuffer: CommandBuffer = acquireCommandBuffer();
  readonly #diagnostics: HastDiagnostic[];
  /** Value nodes edited this pass, whose own return can no longer be sent as a value-only edit. */
  #editedValueNodes: Set<number> | undefined;
  readonly #handle: HastHandle;
  readonly #getSource: () => string;
  readonly #resolver: LazyChildResolver<HastReader, HastNode>;
  readonly #refs: NodeRefs;
  readonly removals: RemovalTracker<HastNode>;
  readonly fileURL: URL | undefined;
  readonly data: Data;
  readonly sourceFormat: SourceFormat;

  constructor(
    handle: HastHandle,
    getSource: () => string,
    fileURL: URL | undefined,
    resolver: LazyChildResolver<HastReader, HastNode>,
    data: Data,
    sourceFormat: SourceFormat,
    diagnostics: HastDiagnostic[],
  ) {
    this.#handle = handle;
    this.#getSource = getSource;
    this.fileURL = fileURL;
    this.#resolver = resolver;
    this.#refs = resolver.refs;
    this.removals = new RemovalTracker(getNodeId, resolver.refs, resolver);
    this.data = data;
    this.sourceFormat = sourceFormat;
    this.#diagnostics = diagnostics;
  }

  get source(): string {
    const value = this.#getSource();
    Object.defineProperty(this, "source", { value, writable: false, enumerable: true });
    return value;
  }

  removeNode(node: HastNode): void {
    const id = requireNid(node, "removeNode", this.#refs);
    this.#commandBuffer.removeNode(id);
    this.removals.removed(id);
  }

  replaceNode(node: HastNode, newNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "replaceNode", this.#refs);
    requireContent("replaceNode", newNode);
    const onlyReplacement = Array.isArray(newNode)
      ? newNode.length === 1
        ? newNode[0]
        : undefined
      : newNode;
    if (
      id === ROOT_NODE_ID &&
      onlyReplacement !== undefined &&
      hastReusedId(onlyReplacement, this.#refs) === id
    ) {
      return;
    }
    this.removals.rebuilt(id, newNode);
    if (onlyReplacement === undefined) {
      const nodes = asArray(newNode);
      if (nodes.length === 0) {
        this.removeNode(node);
        return;
      }
      if (id === ROOT_NODE_ID) throw rootReplacementError(nodes);
      emitHastSequence(this.#commandBuffer, STRUCTURAL_CMD.replace, id, nodes, this.#refs);
      return;
    }
    if (id === ROOT_NODE_ID) {
      emitHastRootReplace(this.#commandBuffer, requireRootReplacement(onlyReplacement), this.#refs);
    } else {
      emitHastTree(this.#commandBuffer, "replace", id, onlyReplacement, this.#refs, true);
    }
    this.#noteEdit(node, id);
  }

  #noteEdit(node: HastNode, id: number): void {
    if (node.type === "text" || node.type === "comment" || node.type === "raw") {
      (this.#editedValueNodes ??= new Set()).add(id);
    }
  }

  /** A visitor's return is its last edit, queued after its context calls. */
  applyReturn(result: HastVisitorResult, nodeId: number, originalNode: HastNode): void {
    // Returning the input node must not overwrite mutations queued through the context.
    if (result == null || result === originalNode) return;
    requireVisitorResult(originalNode, result, false);
    const buffer = this.#commandBuffer;
    if (Array.isArray(result)) {
      if (result.length === 1) {
        this.applyReturn(result[0], nodeId, originalNode);
        return;
      }
      this.removals.returned(nodeId, result);
      if (result.length === 0) buffer.removeNode(nodeId);
      else emitHastSequence(buffer, STRUCTURAL_CMD.replace, nodeId, result, this.#refs);
      return;
    }
    // A value-only edit stands in for the replacement only while nothing else has edited the node.
    if (isTextValueSwap(result, originalNode) && !this.#editedValueNodes?.has(nodeId)) {
      buffer.setProperty(nodeId, "value", (result as { value: string }).value);
      return;
    }
    this.removals.rebuilt(nodeId, result);
    emitHastTree(buffer, "replace", nodeId, result, this.#refs, true);
  }

  #splice(
    anchor: HastNode,
    content: HastContent | HastContent[],
    op: StructuralOp,
    method: string,
  ): void {
    const anchorId = requireNid(anchor, method, this.#refs);
    if (op === "insertBefore" || op === "insertAfter") {
      requireSiblingAnchor(method, anchorId === ROOT_NODE_ID);
    }
    requireContent(method, content);
    const nodes = asArray(content);
    for (const n of nodes) {
      if (getNodeId(n, this.#refs) === anchorId) throw selfInsertError(method);
    }
    this.removals.inserted(anchorId, content);
    if (nodes.length === 1)
      emitHastTree(this.#commandBuffer, op, anchorId, nodes[0]!, this.#refs, true);
    else if (nodes.length > 1)
      emitHastSequence(this.#commandBuffer, STRUCTURAL_CMD[op], anchorId, nodes, this.#refs);
  }

  insertBefore(node: HastNode, newNode: HastContent | HastContent[]): void {
    this.#splice(node, newNode, "insertBefore", "insertBefore");
  }

  insertAfter(node: HastNode, newNode: HastContent | HastContent[]): void {
    this.#splice(node, newNode, "insertAfter", "insertAfter");
  }

  wrapNode(
    node: HastNode,
    parentNode: HastParentContent | RawHastContent | RawHtmlHastContent,
  ): void {
    const id = requireNid(node, "wrapNode", this.#refs);
    requireWrapper(parentNode);
    if (
      typeof (parentNode as RawHastContent).raw === "string" ||
      typeof (parentNode as RawHtmlHastContent).rawHtml === "string"
    ) {
      this.#commandBuffer.wrapNode(id, parentNode as RawHastContent | RawHtmlHastContent);
      return;
    }
    assertHastWrapParent(parentNode as HastContent);
    const wrapperId = getNodeId(parentNode as HastNode, this.#refs);
    if (wrapperId === FOREIGN_REF) throw foreignContentError();
    if (wrapperId !== undefined) throw existingWrapperError();
    this.removals.inserted(id, parentNode);
    emitHastTree(this.#commandBuffer, "wrapNode", id, parentNode as HastContent, this.#refs);
  }

  prependChild(node: HastNode, childNode: HastContent | HastContent[]): void {
    this.#splice(node, childNode, "prependChild", "prependChild");
  }

  appendChild(node: HastNode, childNode: HastContent | HastContent[]): void {
    this.#splice(node, childNode, "appendChild", "appendChild");
  }

  insertChildAt(node: HastNode, index: number, childNode: HastContent | HastContent[]): void {
    const children = "children" in node ? node.children : [];
    const [anchor, op] =
      index <= 0 || children.length === 0
        ? ([node, "prependChild"] as const)
        : index >= children.length
          ? ([node, "appendChild"] as const)
          : ([children[index]!, "insertBefore"] as const);
    this.#splice(anchor, childNode, op, "insertChildAt");
  }

  removeChildAt(node: HastNode, index: number): void {
    const child = "children" in node ? node.children[index] : undefined;
    if (child) this.removeNode(child);
  }

  setProperty(node: HastNode, key: string, value: unknown): void {
    const id = requireNid(node, "setProperty", this.#refs);
    this.#noteEdit(node, id);
    if (key === "children") {
      requireChildren(value);
      this.removals.rebuilt(id, value);
      emitHastChildrenCommand(this.#commandBuffer, id, value as readonly unknown[], this.#refs);
      return;
    }
    if (key === "data") {
      this.#commandBuffer.setProperty(id, key, value != null ? dataJson(value) : null);
      return;
    }
    if (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") {
      // The deprecated property route joins a token list the way hast properties do.
      this.setAttribute(node, key, Array.isArray(value) ? value.join(" ") : value);
      return;
    }
    if (node.type === "element") {
      requireHastPropertyValue(key, value);
      if (Array.isArray(value)) {
        this.#commandBuffer.setTokenListProperty(id, key, encodeTokenList(value));
      } else {
        // NaN drops the attribute, as it does on a built element.
        const dropped = typeof value === "number" && Number.isNaN(value);
        this.#commandBuffer.setProperty(id, key, dropped ? null : value);
      }
      return;
    }

    this.#commandBuffer.setProperty(id, key, value);
  }

  setField(node: HastNode, key: string, value: unknown): void {
    const id = requireNid(node, "setField", this.#refs);
    requireSettableField("setField", key);
    this.#noteEdit(node, id);
    if (key === "children") {
      requireChildren(value);
      this.removals.rebuilt(id, value);
      emitHastChildrenCommand(this.#commandBuffer, id, value as readonly unknown[], this.#refs);
      return;
    }
    if (key === "data") value = value != null ? dataJson(value) : null;
    else if (key === "attributes") value = mdxAttributesJson(value);
    else if (key === "properties") value = propertiesJson(value);
    else if (key === "tagName" && node.type === "element") requireTagName(value);
    this.#commandBuffer.setField(id, key, value);
  }

  setAttribute(node: HastNode, name: string, value: unknown): void {
    const id = requireNid(node, "setAttribute", this.#refs);
    if (node.type !== "mdxJsxFlowElement" && node.type !== "mdxJsxTextElement") {
      const hint = node.type === "element" ? "; set an element's properties with setProperty" : "";
      throw usageError(
        `setAttribute: ${nodeOfType(node.type)} has no attributes, only MDX JSX elements do${hint}`,
        TypeError,
      );
    }
    requireMdxAttributeValue(name, value);
    this.#commandBuffer.setAttribute(id, name, value);
  }

  textContent(node: HastNode): string {
    return textContentHandle(this.#handle, requireNid(node, "textContent", this.#refs));
  }

  parent<N extends Exclude<HastNode, HastRoot>>(node: Readonly<N>): Readonly<HastParents>;
  parent(node: Readonly<HastNode>): Readonly<HastParents> | undefined;
  parent(node: Readonly<HastNode>): Readonly<HastParents> | undefined {
    const parentId = this.#resolver.parentIdOf(requireNid(node as HastNode, "parent", this.#refs));
    if (parentId === undefined) return undefined;
    return this.#resolver.materializeOne(parentId) as HastParents;
  }

  indexOf(node: Readonly<HastNode>): number | undefined {
    return this.#resolver.indexInParent(requireNid(node as HastNode, "indexOf", this.#refs));
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
    const id = node ? getNodeId(node, this.#refs) : undefined;
    this.#diagnostics.push({
      message,
      nodeId: id === FOREIGN_REF ? undefined : id,
      position: node?.position,
      severity,
    });
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
  visit(node: Readonly<N>, ctx: HastVisitorContext): HastVisitorResult | Promise<HastVisitorResult>;
}

type HastVisitorResult = HastNode | HastContent[] | undefined | null | void;

export type HastVisitorFn<N extends HastNode = HastNode> = (
  node: Readonly<N>,
  ctx: HastVisitorContext,
) => HastVisitorResult | Promise<HastVisitorResult>;

export type HastHookFn = (
  root: Readonly<HastRoot>,
  ctx: HastVisitorContext,
) => void | Promise<void>;

export interface HastVisitorInstance {
  /** Plugin-level configuration (e.g. `{ position: true }` to read positions). */
  options?: PluginOptions;
  /** Runs once per document, before the plugin's visitors. Awaited when async. */
  before?: HastHookFn;
  /** Runs once per document, after the plugin's visitors have settled. Awaited
   *  when async. */
  after?: HastHookFn;
  element?: HastFilteredVisitor<Element> | HastFilteredVisitor<Element>[];
  mdxJsxFlowElement?:
    | HastFilteredVisitor<MdxJsxFlowElementHast>
    | HastFilteredVisitor<MdxJsxFlowElementHast>[];
  mdxJsxTextElement?:
    | HastFilteredVisitor<MdxJsxTextElementHast>
    | HastFilteredVisitor<MdxJsxTextElementHast>[];
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

interface ResolvedSubscription {
  nodeType: number;
  tagFilter: string[];
  visitFn: (node: HastNode, ctx: HastVisitorContext) => HastVisitorResult;
}

const FILTERED_METHODS = new Set(["element", "mdxJsxFlowElement", "mdxJsxTextElement"]);

// Cache subscriptions by plugin identity to avoid rebuilding them on every compile.
type CachedSubs = {
  subs: ResolvedSubscription[];
  rustSubs: { nodeType: number; tagFilter: string[] }[];
};
const subscriptionCache: WeakMap<HastVisitorInstance, CachedSubs> = new WeakMap();

export function resolveSubscriptions(plugin: HastVisitorInstance): ResolvedSubscription[] {
  const cached = subscriptionCache.get(plugin);
  if (cached !== undefined) return cached.subs;
  const built = buildSubscriptions(plugin);
  subscriptionCache.set(plugin, built);
  return built.subs;
}

function getRustSubs(plugin: HastVisitorInstance): { nodeType: number; tagFilter: string[] }[] {
  const cached = subscriptionCache.get(plugin);
  if (cached !== undefined) return cached.rustSubs;
  const built = buildSubscriptions(plugin);
  subscriptionCache.set(plugin, built);
  return built.rustSubs;
}

function isFilteredVisitor(value: unknown): value is HastFilteredVisitor {
  return (
    value !== null &&
    typeof value === "object" &&
    Array.isArray((value as { filter?: unknown }).filter) &&
    typeof (value as { visit?: unknown }).visit === "function"
  );
}

// Validate before NAPI so errors name the public visitor contract.
function malformedFilteredVisitorError(plugin: HastVisitorInstance, methodName: string): Error {
  const name = (plugin as { name?: unknown }).name;
  const pluginName = typeof name === "string" && name !== "" ? name : "(unnamed)";
  return new Error(
    `hast plugin "${pluginName}": "${methodName}" visitors filter by tag/component name, ` +
      `so each must be an object { filter: string[], visit: function } (or an array of those). ` +
      `Use filter: [] to visit every "${methodName}" node.`,
  );
}

function buildSubscriptions(plugin: HastVisitorInstance): CachedSubs {
  const subs: ResolvedSubscription[] = [];

  for (const [methodName, nodeType] of Object.entries(METHOD_TO_TYPE)) {
    const value = plugin[methodName as keyof HastVisitorInstance];
    if (value === undefined) continue;

    if (FILTERED_METHODS.has(methodName)) {
      const items = Array.isArray(value) ? value : [value];
      for (const fv of items) {
        if (!isFilteredVisitor(fv)) throw malformedFilteredVisitorError(plugin, methodName);
        subs.push({
          nodeType,
          tagFilter: fv.filter,
          visitFn: fv.visit as ResolvedSubscription["visitFn"],
        });
      }
    } else {
      subs.push({ nodeType, tagFilter: [], visitFn: value as ResolvedSubscription["visitFn"] });
    }
  }

  const rustSubs = subs.map((s) => ({ nodeType: s.nodeType, tagFilter: s.tagFilter }));
  return { subs, rustSubs };
}

const METHOD_TO_TYPE: Record<string, number> = Object.fromEntries(
  [...VISITOR_KEYS].map((name) => [name, NAME_TO_TYPE[name]!] as const),
);

// Explicit field checks avoid allocating Object.keys on the per-text-node hot path.
function isTextValueSwap(result: HastNode, original: HastNode): boolean {
  if (result.type !== original.type) return false;
  if (result.type !== "text" && result.type !== "comment" && result.type !== "raw") return false;
  const r = result as unknown as Record<string, unknown>;
  if (typeof r.value !== "string") return false;
  return (
    r.children === undefined &&
    r.position === undefined &&
    r.data === undefined &&
    r.tagName === undefined &&
    r.properties === undefined &&
    r.name === undefined &&
    r.attributes === undefined
  );
}

function dispatchMatches(
  matchBuf: Uint8Array,
  subs: ResolvedSubscription[],
  ctx: HastVisitorContextImpl,
  resolver: HastLazyChildResolver,
): Promise<void>[] | null {
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = matchView.getUint32(0, true);
  const wire: WalkWire = { view: matchView, buf: matchBuf, resolver };
  let settling: Promise<void>[] | null = null;

  for (let i = 0; i < matchCount; i++) {
    const indexBase = 4 + i * 10;
    const nodeId = matchView.getUint32(indexBase, true);
    const subIndex = matchBuf[indexBase + 4]!;
    const dataOffset = matchView.getUint32(indexBase + 6, true);

    const sub = subs[subIndex]!;
    const node = readMatchedNode(wire, dataOffset, nodeId, sub.nodeType);
    const result = sub.visitFn(node, ctx);
    if (result instanceof Promise) {
      // An async visitor's return is its last edit, so it queues when the visitor settles.
      (settling ??= []).push(result.then((settled) => ctx.applyReturn(settled, nodeId, node)));
    } else if (result !== undefined) {
      ctx.applyReturn(result, nodeId, node);
    }
  }

  return settling;
}

/**
 * Walk a handle's arena in Rust, dispatch matched nodes to JS visitor functions,
 * and apply mutations back to the handle. No arena buffers cross NAPI.
 *
 * Returns how many nodes had edits dropped because they had left the tree by the
 * time the edits applied (the caller warns when non-zero), or a Promise of that
 * count if any visitor is async.
 */
export function visitHastHandle(
  handle: HastHandle,
  plugin: HastVisitorInstance,
  subs: ResolvedSubscription[],
  source: string | (() => string),
  fileURL: URL | undefined,
  data: Data = {},
  sourceFormat: SourceFormat = "markdown",
  diagnostics: HastDiagnostic[] = [],
): number | Promise<number> {
  const result = visitHastHandleCollect(
    handle,
    plugin,
    subs,
    source,
    fileURL,
    data,
    sourceFormat,
    diagnostics,
  );
  if (result instanceof Promise) {
    return result.then((commands) => applyCollectedCommands(handle, commands, plugin));
  }
  return applyCollectedCommands(handle, result, plugin);
}

function applyCollectedCommands(
  handle: HastHandle,
  commands: Uint8Array,
  plugin: HastVisitorInstance,
): number {
  if (commands.length === 0) return 0;
  markHandleMutated(handle);
  return withPluginErrors(plugin, () => applyCommandsToHandle(handle, commands))?.length ?? 0;
}

// Collect the last plugin’s commands so the caller can fuse their apply with rendering or compilation.
export function visitHastHandleCollect(
  handle: HastHandle,
  plugin: HastVisitorInstance,
  subs: ResolvedSubscription[],
  source: string | (() => string),
  fileURL: URL | undefined,
  data: Data = {},
  sourceFormat: SourceFormat = "markdown",
  diagnostics: HastDiagnostic[] = [],
): Uint8Array | Promise<Uint8Array> {
  const getSource = typeof source === "function" ? source : () => source;
  const resolver = new HastLazyChildResolver(handle);
  const ctx = new HastVisitorContextImpl(
    handle,
    getSource,
    fileURL,
    resolver,
    data,
    sourceFormat,
    diagnostics,
  );
  const rustSubs = getRustSubs(plugin);
  const settling = dispatchMatches(walkHandle(handle, rustSubs), subs, ctx, resolver);
  if (settling) return Promise.all(settling).then(() => finishHastVisit(handle, ctx, plugin));
  return finishHastVisit(handle, ctx, plugin);
}

function finishHastVisit(
  handle: HastHandle,
  ctx: HastVisitorContextImpl,
  plugin: HastVisitorInstance,
): Uint8Array {
  ctx.removals.warn(plugin, (ids) => nodeLabels(handle, ids));
  return takeCommands(ctx.getCommandBuffer());
}

const HAST_ROOT_SUBS: { nodeType: number; tagFilter: string[] }[] = [
  { nodeType: HAST_ROOT, tagFilter: [] },
];

/** Hooks run in separate passes so each sees mutations from the preceding pass. */
export function visitHastHookCollect(
  handle: HastHandle,
  plugin: HastVisitorInstance,
  hook: HastHookFn,
  source: string | (() => string),
  fileURL: URL | undefined,
  data: Data = {},
  sourceFormat: SourceFormat = "markdown",
  diagnostics: HastDiagnostic[] = [],
): Uint8Array | Promise<Uint8Array> {
  const getSource = typeof source === "function" ? source : () => source;
  const resolver = new HastLazyChildResolver(handle);
  const ctx = new HastVisitorContextImpl(
    handle,
    getSource,
    fileURL,
    resolver,
    data,
    sourceFormat,
    diagnostics,
  );
  const matchBuf = walkHandle(handle, HAST_ROOT_SUBS);
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  if (matchView.getUint32(0, true) === 0) return finishHastVisit(handle, ctx, plugin);

  const wire: WalkWire = { view: matchView, buf: matchBuf, resolver };
  const root = readMatchedNode(
    wire,
    matchView.getUint32(10, true),
    matchView.getUint32(4, true),
    HAST_ROOT,
  ) as HastRoot;

  const result = hook.call(plugin, root, ctx);
  if (result instanceof Promise) return result.then(() => finishHastVisit(handle, ctx, plugin));
  return finishHastVisit(handle, ctx, plugin);
}

export function visitHastHook(
  handle: HastHandle,
  plugin: HastVisitorInstance,
  hook: HastHookFn,
  source: string | (() => string),
  fileURL: URL | undefined,
  data: Data = {},
  sourceFormat: SourceFormat = "markdown",
  diagnostics: HastDiagnostic[] = [],
): number | Promise<number> {
  const result = visitHastHookCollect(
    handle,
    plugin,
    hook,
    source,
    fileURL,
    data,
    sourceFormat,
    diagnostics,
  );
  if (result instanceof Promise) {
    return result.then((commands) => applyCollectedCommands(handle, commands, plugin));
  }
  return applyCollectedCommands(handle, result, plugin);
}
