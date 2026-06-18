import { materializeHastNode, type HastNode } from "./hast-materializer.js";
import type { HastRaw, MdxJsxAttributeUnion, Position, Data } from "../types.js";
import type {
  Element,
  Text,
  Comment,
  Doctype,
  Parents as HastParents,
  Root as HastRoot,
} from "hast";
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
import {
  TYPE_NAMES,
  NAME_TO_TYPE,
  VISITOR_KEYS,
  HAST_OPSTREAM_TYPES,
} from "./generated/node-types.js";
import { CommandBuffer, type StructuralOp } from "../command-buffer.js";
import {
  OpWriter,
  OF_VALUE,
  OF_TAGNAME,
  OF_NAME,
  OF_EXPLICIT,
  PROP_STRING,
  PROP_BOOL_TRUE,
  PROP_BOOL_FALSE,
  PROP_SPACE_SEP,
  PROP_INT,
  emitMdxAttr,
} from "../op-stream.js";
import { restorePhantomSpaces } from "../phantom.js";
import { decodeMdxJsxAttr } from "../mdx-attr.js";
import { decodeElementProp } from "./element-props.js";
import { readPosition, rstr } from "../wire-read.js";
import {
  walkHandle,
  applyCommandsToHandle,
  textContentHandle,
  parseExpression as napiParseExpression,
  parseEsm as napiParseEsm,
} from "#binding";

import {
  asArray,
  makeRequireNid,
  mergeAndReset,
  unencodableContentError,
} from "../visitor-shared.js";
import { LazyChildResolver, markHandleMutated } from "../lazy-child-resolver.js";
import { HastChildStub } from "./child-stub.js";
import type { HastHandle } from "../handles.js";

export type { HastHandle };

type NapiParseFn = (source: string) => string | null;

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
  /**
   * Document-level data bag, shared across every plugin in the compile and
   * across the mdast→hast phase boundary. Mutate keys directly
   * (`ctx.data.foo = x`); the bag itself isn't reassignable. Values are kept
   * on the JS side, so any value is allowed, including functions and class
   * instances. Returned to the caller as `result.data`.
   */
  readonly data: Data;
  removeNode(node: Readonly<HastNode>): void;
  replaceNode(node: Readonly<HastNode>, newNode: HastContent): void;
  insertBefore(node: Readonly<HastNode>, newNode: HastContent | HastContent[]): void;
  insertAfter(node: Readonly<HastNode>, newNode: HastContent | HastContent[]): void;
  /**
   * Wrap `node` in `parentNode`, making it `parentNode`'s first child. Any
   * children `parentNode` declares are kept after it, so a `div` with an anchor
   * child wraps a heading as `div > [heading, anchor]`.
   */
  wrapNode(node: Readonly<HastNode>, parentNode: HastContent): void;
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

/**
 * Arena identity of a node, rejecting impostors — the one place the
 * spread/identity invariant is enforced. A spread copy of a matched node or
 * stub must read as NEW content: trusting a copied id would splice the
 * original in as a ref and drop the copy's edits. Walk elements carry their
 * id in a private field behind `instanceof` (spread copies fail the check);
 * other walk-built nodes are keyed in the WeakMap (invisible to spread);
 * `HastChildStub`s (enumerable `_id`, but that key is ignored on plain
 * objects) are recognized by `instanceof`. Plain objects are trusted only via
 * the WeakMap or a NON-enumerable `_nodeId` (the materializers' convention,
 * which spread cannot copy).
 */
function nid(node: HastNode): number | undefined {
  if (node instanceof WalkElement) return node._nid;
  if (node instanceof HastChildStub) return node._id;
  const id = nodeIdMap.get(node);
  if (id !== undefined) return id;
  const d = Object.getOwnPropertyDescriptor(node, "_nodeId");
  return d !== undefined && !d.enumerable ? (d.value as number) : undefined;
}

const requireNid = makeRequireNid(nid);

/** New content for a HAST structural mutation. Unlike [`MdastContent`], HAST has
 *  a `raw` node type, so it needs no raw/rawHtml escape hatch. */
export type HastContent = HastNode;

function hastReusedId(node: unknown): number | undefined {
  if (node === null || typeof node !== "object") return undefined;
  const id = nid(node as HastNode);
  return typeof id === "number" ? id : undefined;
}

// Reused across replacements in a pass — see the note on `mdastWriter`.
const hastWriter = new OpWriter();

/** Compile a set-children payload: a root-wrapped child list, the shape
 *  `Patch::SetChildren` splices in. Reused children become refs. */
function compileHastChildrenToOpstream(children: unknown): Uint8Array | null {
  if (!Array.isArray(children)) return null;
  hastWriter.begin();
  try {
    hastWriter.open(NAME_TO_TYPE.root!);
    for (const c of children) {
      if (!emitHastOp(hastWriter, c, false)) return null;
    }
    hastWriter.close();
    return hastWriter.take();
  } finally {
    hastWriter.end();
  }
}

/** Encode `node` as the `op` structural command. HAST content is always a
 *  declarative node (no raw escape hatch), so it compiles to the op-stream or
 *  it's a hard error — the op-stream is the only structural encoding. The
 *  switch stays inline so the buffer calls are monomorphic (computed method
 *  names defeat inline caches on this warm path). */
function emitHastTree(buffer: CommandBuffer, op: StructuralOp, id: number, node: HastNode): void {
  const ops = compileHastToOpstream(node);
  if (ops === null) throw unencodableContentError(node);
  switch (op) {
    case "replace":
      return buffer.replaceOpstream(id, ops);
    case "insertBefore":
      return buffer.insertBeforeOpstream(id, ops);
    case "insertAfter":
      return buffer.insertAfterOpstream(id, ops);
    case "prependChild":
      return buffer.prependChildOpstream(id, ops);
    case "appendChild":
      return buffer.appendChildOpstream(id, ops);
    case "wrapNode":
      return buffer.wrapNodeOpstream(id, ops);
  }
}

/**
 * Compile a declarative HAST replacement tree to the op-stream — the only
 * structural encoding. Reused nodes become `ref`s (transparent passthrough).
 * Returns null when the tree holds a type the replay can't reproduce
 * identically; the caller throws.
 */
function compileHastToOpstream(root: unknown): Uint8Array | null {
  hastWriter.begin();
  try {
    if (!emitHastOp(hastWriter, root, true)) return null;
    return hastWriter.take();
  } finally {
    hastWriter.end();
  }
}

function emitHastOp(w: OpWriter, node: unknown, isRoot: boolean): boolean {
  if (node === null || typeof node !== "object") return false;
  if (!isRoot) {
    const id = hastReusedId(node);
    if (id !== undefined) {
      w.ref(id);
      return true;
    }
  }
  const n = node as Record<string, unknown>;
  const type = HAST_OPSTREAM_TYPES[n.type as string];
  if (type === undefined) return false;
  w.open(type);
  if (type === HAST_ELEMENT) {
    w.str(OF_TAGNAME, typeof n.tagName === "string" ? n.tagName : "div");
    const props = n.properties;
    if (props !== null && typeof props === "object") {
      for (const key in props as Record<string, unknown>) {
        emitHastProp(w, key, (props as Record<string, unknown>)[key]);
      }
    }
  } else if (type === HAST_MDX_JSX_ELEMENT || type === HAST_MDX_JSX_TEXT_ELEMENT) {
    // Name falls back to tagName, matching `encode_hast_js_node_data`.
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
  if (n.data != null) w.data(n.data);
  const children = n.children;
  if (Array.isArray(children)) {
    for (const c of children) if (!emitHastOp(w, c, false)) return false;
  }
  w.close();
  return true;
}

/** Emit one element property, mirroring `encode_hast_js_node_data` exactly:
 *  bool/string/number/array → kind; null/object → skip. */
function emitHastProp(w: OpWriter, name: string, value: unknown): void {
  if (value === true) w.prop(name, PROP_BOOL_TRUE, "");
  else if (value === false) w.prop(name, PROP_BOOL_FALSE, "");
  else if (typeof value === "string") w.prop(name, PROP_STRING, value);
  else if (typeof value === "number") w.prop(name, PROP_INT, String(value));
  else if (Array.isArray(value))
    w.prop(name, PROP_SPACE_SEP, value.filter((v) => typeof v === "string").join(" "));
}

class HastVisitorContextImpl implements HastVisitorContext {
  readonly #commandBuffer: CommandBuffer = new CommandBuffer();
  readonly #diagnostics: HastDiagnostic[] = [];
  /** Track accumulated node state for multiple setProperty calls on the same node. */
  readonly #pendingNodes: Map<number, HastNode> = new Map();
  readonly #handle: HastHandle;
  readonly #getSource: () => string;
  readonly #resolver: LazyChildResolver<HastReader, HastNode>;
  /** One canonical object per parent id, so visitors can dedupe by identity.
   *  Null until the first `parent()` call; most passes never make one. */
  #parentsById: Map<number, HastNode> | null = null;
  readonly fileURL: URL | undefined;
  readonly data: Data;

  constructor(
    handle: HastHandle,
    getSource: () => string,
    fileURL: URL | undefined,
    resolver: LazyChildResolver<HastReader, HastNode>,
    data: Data,
  ) {
    this.#handle = handle;
    this.#getSource = getSource;
    this.fileURL = fileURL;
    this.#resolver = resolver;
    this.data = data;
  }

  get source(): string {
    const value = this.#getSource();
    Object.defineProperty(this, "source", { value, writable: false, enumerable: true });
    return value;
  }

  removeNode(node: HastNode): void {
    this.#commandBuffer.removeNode(requireNid(node, "removeNode"));
  }

  replaceNode(node: HastNode, newNode: HastContent): void {
    const id = requireNid(node, "replaceNode");
    emitHastTree(this.#commandBuffer, "replace", id, newNode);
    // Track the replacement so a later mdxJsx setProperty can fold into it.
    this.#pendingNodes.set(id, newNode);
  }

  insertBefore(node: HastNode, newNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "insertBefore");
    for (const n of asArray(newNode)) emitHastTree(this.#commandBuffer, "insertBefore", id, n);
  }

  insertAfter(node: HastNode, newNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "insertAfter");
    for (const n of asArray(newNode)) emitHastTree(this.#commandBuffer, "insertAfter", id, n);
  }

  wrapNode(node: HastNode, parentNode: HastContent): void {
    const id = requireNid(node, "wrapNode");
    emitHastTree(this.#commandBuffer, "wrapNode", id, parentNode);
  }

  prependChild(node: HastNode, childNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "prependChild");
    for (const n of asArray(childNode)) emitHastTree(this.#commandBuffer, "prependChild", id, n);
  }

  appendChild(node: HastNode, childNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "appendChild");
    for (const n of asArray(childNode)) emitHastTree(this.#commandBuffer, "appendChild", id, n);
  }

  insertChildAt(node: HastNode, index: number, childNode: HastContent | HastContent[]): void {
    const children = "children" in node ? node.children : [];
    if (index <= 0 || children.length === 0) {
      this.prependChild(node, childNode);
    } else if (index >= children.length) {
      this.appendChild(node, childNode);
    } else {
      this.insertBefore(children[index]!, childNode);
    }
  }

  removeChildAt(node: HastNode, index: number): void {
    const child = "children" in node ? node.children[index] : undefined;
    if (child) this.removeNode(child);
  }

  setProperty(node: HastNode, key: string, value: unknown): void {
    const id = requireNid(node, "setProperty");
    if (key === "children") {
      // children is structural: set-children keeps the node and swaps only its
      // child list (reused children keep their id).
      const ops = compileHastChildrenToOpstream(value);
      if (!ops) throw unencodableContentError(value);
      this.#commandBuffer.setChildrenOpstream(id, ops);
      return;
    }
    if (key === "data") {
      this.#commandBuffer.setProperty(id, key, value != null ? JSON.stringify(value) : null);
      return;
    }
    if (node.type === "element") {
      this.#commandBuffer.setProperty(id, key, value);
      return;
    }

    if (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") {
      // MDX JSX nodes carry `attributes`, not `properties`. If a replacement is
      // already queued for this node, fold the attribute into it so the change
      // survives the rebuild. This spreads the queued replacement object, not
      // the matched node, so it never forces the matched node's children to
      // materialize.
      const pending = this.#pendingNodes.get(id) as
        | MdxJsxFlowElementHast
        | MdxJsxTextElementHast
        | undefined;
      if (pending !== undefined) {
        const updated = { ...pending };
        const attrs: MdxJsxAttributeUnion[] = [...(updated.attributes ?? [])];
        const idx = attrs.findIndex((a) => a.type === "mdxJsxAttribute" && a.name === key);
        if (idx !== -1) attrs.splice(idx, 1);
        // Arrays space-join, matching the binary path's PROP_SPACE_SEP encoding
        // (hast convention for list-valued properties like className).
        const attrValue =
          value === true || value === null || value === undefined
            ? null
            : typeof value === "string"
              ? value
              : Array.isArray(value)
                ? value.join(" ")
                : String(value);
        attrs.push({ type: "mdxJsxAttribute", name: key, value: attrValue });
        updated.attributes = attrs;
        this.replaceNode(node, updated);
        return;
      }
      // Binary attribute upsert in the arena's type_data — no child
      // materialization. Rust maps the value-type to a boolean (true/null) or
      // literal (string/number/false) attribute, mirroring the fold path above.
      this.#commandBuffer.setProperty(id, key, value);
      return;
    }

    // Text-like nodes (text, comment, raw, expressions, esm): Rust handles
    // `value` directly on these types.
    this.#commandBuffer.setProperty(id, key, value);
  }

  textContent(node: HastNode): string {
    return textContentHandle(this.#handle, requireNid(node, "textContent"));
  }

  parent<N extends Exclude<HastNode, HastRoot>>(node: Readonly<N>): Readonly<HastParents>;
  parent(node: Readonly<HastNode>): Readonly<HastParents> | undefined;
  parent(node: Readonly<HastNode>): Readonly<HastParents> | undefined {
    const parentId = this.#resolver.parentIdOf(requireNid(node as HastNode, "parent"));
    if (parentId === undefined) return undefined;
    const byId = (this.#parentsById ??= new Map());
    let parent = byId.get(parentId);
    if (parent === undefined) {
      parent = this.#resolver.materializeOne(parentId);
      byId.set(parentId, parent);
    }
    return parent as HastParents;
  }

  indexOf(node: Readonly<HastNode>): number | undefined {
    return this.#resolver.indexInParent(requireNid(node as HastNode, "indexOf"));
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

interface ResolvedSubscription {
  nodeType: number;
  tagFilter: string[];
  visitFn: (node: HastNode, ctx: HastVisitorContext) => HastNode | void;
}

/** Node types that use filtered visitors (have tag/component names). */
const FILTERED_METHODS = new Set(["element", "mdxJsxFlowElement", "mdxJsxTextElement"]);

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

/** Visitor method name → node-type tag (method names are the subscribable AST names). */
const METHOD_TO_TYPE: Record<string, number> = Object.fromEntries(
  [...VISITOR_KEYS].map((name) => [name, NAME_TO_TYPE[name]!] as const),
);

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
    const name = rstr(buf, pos, nameLen);
    pos += nameLen;
    const kind = buf[pos]!;
    pos += 1;
    const valLen = view.getUint16(pos, true);
    pos += 2;
    const valStr = rstr(buf, pos, valLen);
    pos += valLen;
    properties[name] = decodeElementProp(kind, valStr);
  }
  return properties;
}

/** Build the child-stub list for a matched node from the wire's `[child_ids]
 *  [child_types]` blocks — no arena snapshot. The seal check still applies:
 *  post-pass ids are stale, and a stub built from them could later splice the
 *  wrong node as a ref. */
function readChildStubs(
  view: DataView,
  buf: Uint8Array,
  idsPos: number,
  typesPos: number,
  count: number,
  resolver: HastLazyChildResolver,
): HastNode[] {
  resolver.assertUnsealed();
  const stubs: HastNode[] = new Array(count);
  for (let i = 0; i < count; i++) {
    stubs[i] = new HastChildStub(
      resolver,
      view.getUint32(idsPos + i * 4, true),
      buf[typesPos + i]!,
    ) as unknown as HastNode;
  }
  return stubs;
}

type HastProperties = Record<string, string | number | boolean | string[]>;

// Shared own-getter descriptors for WalkElement's lazy fields, populated in
// its static block so the getters can read the private wire fields.
let WALK_PROPS_DESC!: PropertyDescriptor;
let WALK_CHILDREN_DESC!: PropertyDescriptor;

/**
 * Walk-path element. Spread-correctness requires `properties`/`children` as
 * own enumerable keys (`{ ...node }` copies nothing else), but construction
 * runs per matched element, so everything stays off the expensive paths:
 * wire state in private fields (plain stores, invisible to spread — a WeakMap
 * entry per element caused major-GC ephemeron stalls at this volume), shared
 * getter functions instead of per-node closures, at most one define per lazy
 * field, and `instanceof` gating identity so copies read as new content.
 */
class WalkElement {
  readonly type = "element" as const;
  tagName: string;
  declare properties: HastProperties;
  declare position?: Position;
  declare data?: Record<string, unknown>;
  declare children?: HastNode[];

  readonly #nodeId: number;
  #view: DataView;
  #buf: Uint8Array;
  #propsPos: number;
  #childIdsPos: number;
  #childTypesPos: number;
  #childCount: number;
  #resolver: HastLazyChildResolver;

  constructor(
    tagName: string,
    nodeId: number,
    view: DataView,
    buf: Uint8Array,
    propsPos: number,
    propCount: number,
    childIdsPos: number,
    childTypesPos: number,
    childCount: number,
    resolver: HastLazyChildResolver,
  ) {
    this.tagName = tagName;
    this.#nodeId = nodeId;
    this.#view = view;
    this.#buf = buf;
    this.#propsPos = propsPos;
    this.#childIdsPos = childIdsPos;
    this.#childTypesPos = childTypesPos;
    this.#childCount = childCount;
    this.#resolver = resolver;
    if (propCount === 0) {
      this.properties = {};
    } else {
      Object.defineProperty(this, "properties", WALK_PROPS_DESC);
    }
    if (childCount === 0) {
      this.children = [];
    } else {
      Object.defineProperty(this, "children", WALK_CHILDREN_DESC);
    }
  }

  /** @internal */
  get _nid(): number {
    return this.#nodeId;
  }

  static {
    WALK_PROPS_DESC = {
      enumerable: true,
      configurable: true,
      get(this: WalkElement): HastProperties {
        const val = decodeProperties(this.#view, this.#buf, this.#propsPos);
        Object.defineProperty(this, "properties", {
          value: val,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        return val;
      },
    };
    WALK_CHILDREN_DESC = {
      enumerable: true,
      configurable: true,
      get(this: WalkElement): HastNode[] {
        const val = readChildStubs(
          this.#view,
          this.#buf,
          this.#childIdsPos,
          this.#childTypesPos,
          this.#childCount,
          this.#resolver,
        );
        Object.defineProperty(this, "children", {
          value: val,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        return val;
      },
    };
  }
}

/** Read the tail of a matched element node (tag + properties).
 *  Common prelude (data/position/children) is already consumed by `readMatchedNode`. */
function readElementFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  resolver: HastLazyChildResolver,
  position: Position | undefined,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  data: Record<string, unknown> | null,
): HastNode {
  let pos = offset;

  // Eager: tagName (almost always accessed by visitors)
  const tagLen = view.getUint16(pos, true);
  pos += 2;
  const tagName = rstr(buf, pos, tagLen);
  pos += tagLen;

  const propCount = view.getUint16(pos, true);
  const node = new WalkElement(
    tagName,
    nodeId,
    view,
    buf,
    pos,
    propCount,
    childIdsPos,
    childTypesPos,
    childCount,
    resolver,
  );
  if (position !== undefined) node.position = position;
  if (data !== null) node.data = data;
  return node as unknown as HastNode;
}

/** Value-carrying types read by `readTextFromBinary` (tag → AST name). */
const TEXT_NODE_TYPES: Record<number, string> = Object.fromEntries(
  ["text", "comment", "raw", "mdxFlowExpression", "mdxTextExpression", "mdxjsEsm"].map(
    (name) => [NAME_TO_TYPE[name]!, name] as const,
  ),
);

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
  const rawValue = rstr(buf, offset + 4, valLen);
  // MDX flow/text expressions store phantom-space sentinels; restore them so
  // the value matches the reader path. ESM and plain text keep their value.
  const value =
    nodeType === HAST_MDX_FLOW_EXPRESSION || nodeType === HAST_MDX_TEXT_EXPRESSION
      ? restorePhantomSpaces(rawValue)
      : rawValue;
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

function readMdxJsxFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver: HastLazyChildResolver,
  position: Position | undefined,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  data: Record<string, unknown> | null,
): HastNode {
  let pos = offset;

  const nameLen = view.getUint16(pos, true);
  pos += 2;
  const name = nameLen > 0 ? rstr(buf, pos, nameLen) : null;
  pos += nameLen;

  // Attributes: [kind: u8][nameLen: u16][name][valLen: u32][val]
  const attrCount = view.getUint16(pos, true);
  pos += 2;
  const attributes: MdxJsxAttributeUnion[] = [];
  for (let i = 0; i < attrCount; i++) {
    const kind = buf[pos]!;
    pos += 1;
    const attrNameLen = view.getUint16(pos, true);
    pos += 2;
    const attrName = rstr(buf, pos, attrNameLen);
    pos += attrNameLen;
    const attrValLen = view.getUint32(pos, true);
    pos += 4;
    const attrVal = rstr(buf, pos, attrValLen);
    pos += attrValLen;
    attributes.push(decodeMdxJsxAttr(kind, attrName, attrVal));
  }

  const typeName = nodeType === HAST_MDX_JSX_ELEMENT ? "mdxJsxFlowElement" : "mdxJsxTextElement";
  const base: Record<string, unknown> = { type: typeName, name, attributes };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  nodeIdMap.set(base, nodeId);
  makeLazyChildren(base, view, buf, childIdsPos, childTypesPos, childCount, resolver);
  return base as unknown as HastNode;
}

function readMatchedNode(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver: HastLazyChildResolver,
): HastNode {
  let pos = offset;

  // Shared prelude (matches serialize_hast_node_inline / serialize_mdast_node_inline):
  //   [data_len: u32][data_bytes][position: 24B][child_count: u32][child_ids: N×u32][child_types: N×u8]
  const dataLen = view.getUint32(pos, true);
  pos += 4;
  let data: Record<string, unknown> | null = null;
  if (dataLen > 0) {
    const jsonStr = rstr(buf, pos, dataLen);
    try {
      data = JSON.parse(jsonStr) as Record<string, unknown>;
    } catch (err) {
      if (process.env.NODE_ENV !== "production") {
        console.warn(`readMatchedNode: malformed node_data for nodeId=${nodeId}`, err);
      }
    }
    pos += dataLen;
  }

  const position = readPosition(view, pos);
  pos += 24;

  const childCount = view.getUint32(pos, true);
  pos += 4;
  // Ids/types decode lazily with `.children` — most matched nodes never read them.
  const childIdsPos = pos;
  pos += childCount * 4;
  const childTypesPos = pos;
  pos += childCount;

  // Dispatch to type-specific tail (pos now sits at the type-specific section)
  if (nodeType === HAST_ELEMENT) {
    return readElementFromBinary(
      view,
      buf,
      pos,
      nodeId,
      resolver,
      position,
      childIdsPos,
      childTypesPos,
      childCount,
      data,
    );
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
      childIdsPos,
      childTypesPos,
      childCount,
      data,
    );
  }
  // Fallback (e.g. doctype): minimal node carrying whatever prelude data we found
  const base: Record<string, unknown> = { type: TYPE_NAMES[nodeType] ?? `unknown(${nodeType})` };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  const node = base as unknown as HastNode;
  nodeIdMap.set(node, nodeId);
  return node;
}

class HastLazyChildResolver extends LazyChildResolver<HastReader, HastNode> {
  protected override createReader(wire: Uint8Array): HastReader {
    return new HastReader(wire);
  }

  protected override materializeNode(reader: HastReader, nodeId: number): HastNode {
    return materializeHastNode(reader, nodeId);
  }

  protected override readParentId(reader: HastReader, nodeId: number): number {
    return reader.getParentId(nodeId);
  }

  protected override readChildIds(reader: HastReader, nodeId: number): number[] {
    return reader.getChildIds(nodeId);
  }
}

/** Install `children` as an own enumerable getter (spread must carry it),
 *  self-replacing with the one stable stub array on first read. One closure
 *  and one define per node — installing the wire locals as hidden slots
 *  instead measurably regressed every matching pipeline. */
function makeLazyChildren(
  node: object,
  view: DataView,
  buf: Uint8Array,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  resolver: HastLazyChildResolver,
): void {
  Object.defineProperty(node, "children", {
    get(this: object): HastNode[] {
      const val = readChildStubs(view, buf, childIdsPos, childTypesPos, childCount, resolver);
      Object.defineProperty(this, "children", {
        value: val,
        writable: true,
        enumerable: true,
        configurable: true,
      });
      return val;
    },
    enumerable: true,
    configurable: true,
  });
}

/** A result that is the same object as the input node is a no-op, so context
 *  mutations (e.g. setProperty) are not clobbered. */
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
  emitHastTree(returnBuffer, "replace", nodeId, result);
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
  resolver: HastLazyChildResolver,
): { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[] | null {
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = matchView.getUint32(0, true);
  let deferred:
    | { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[]
    | null = null;

  for (let i = 0; i < matchCount; i++) {
    const indexBase = 4 + i * 10;
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

/**
 * Walk a handle's arena in Rust, dispatch matched nodes to JS visitor functions,
 * and apply mutations back to the handle. No arena buffers cross NAPI.
 *
 * Returns the number of patches dropped because their target was removed or
 * replaced earlier in the same pass (the caller warns when non-zero), or a
 * Promise of that count if any visitor is async.
 */
export function visitHastHandle(
  handle: HastHandle,
  plugin: HastVisitorInstance,
  subs: ResolvedSubscription[],
  source: string | (() => string),
  fileURL: URL | undefined,
  data: Data = {},
): number | Promise<number> {
  const getSource = typeof source === "function" ? source : () => source;
  const resolver = new HastLazyChildResolver(handle);
  const ctx = new HastVisitorContextImpl(handle, getSource, fileURL, resolver, data);
  const returnBuffer = new CommandBuffer();
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
          emitHastTree(returnBuffer, "replace", nodeId, result);
        }
      }
      // Mutations land next, renumbering the arena: snapshots taken after
      // this point would resolve match-time child ids against wrong nodes.
      resolver.seal();
      return applyMutations(handle, returnBuffer, ctx);
    });
  }

  resolver.seal();
  return applyMutations(handle, returnBuffer, ctx);
}

/** Returns the number of patches dropped as stranded (0 when none). */
function applyMutations(
  handle: HastHandle,
  returnBuffer: CommandBuffer,
  ctx: HastVisitorContextImpl,
): number {
  const { merged, hasMutations } = mergeAndReset(returnBuffer, ctx);
  if (hasMutations) {
    markHandleMutated(handle);
    return applyCommandsToHandle(handle, merged);
  }
  return 0;
}
