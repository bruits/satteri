import { materializeHastNode, type HastNode } from "./hast-materializer.js";
import type { HastRaw, MdxJsxAttributeUnion, Position, Data, SourceFormat } from "../types.js";
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
} from "./hast-reader.js";
import {
  TYPE_NAMES,
  NAME_TO_TYPE,
  VISITOR_KEYS,
  HAST_OPSTREAM_TYPES,
} from "./generated/node-types.js";
import {
  acquireCommandBuffer,
  releaseCommandBuffer,
  CommandBuffer,
  STRUCTURAL_CMD,
  type StructuralOp,
} from "../command-buffer.js";
import { CMD_SET_CHILDREN } from "../generated/wire-constants.js";
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
import {
  decodeWalkElementProps,
  readWalkElementTag,
  readWalkHastValue,
  readWalkMdxJsx,
  walkElementPropCount,
  walkElementPropsAt,
} from "./generated/walk-decode.js";
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
  type PluginOptions,
  ROOT_NODE_ID,
  requireRootReplacement,
  reuseAncestorError,
  reuseCycleError,
  rootReplacementError,
  crossPipelineForeign,
  FOREIGN_REF,
  type NodeRefs,
  unencodableContentError,
} from "../visitor-shared.js";
import {
  LazyChildResolver,
  markHandleMutated,
  registerEpochCacheSlot,
  type EpochCache,
} from "../lazy-child-resolver.js";
import { HastChildStub } from "./child-stub.js";
import type { AnyHandle, HastHandle } from "../handles.js";

export type { HastHandle };

type NapiParseFn = (source: string) => string | null;

/** ESTree-compatible Program node returned by `parseExpression()`. */
export type EstreeProgram = Program;

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
function nid(node: HastNode, refs: NodeRefs): number | undefined {
  if (node instanceof WalkElement) return node._refs === refs ? node._nid : FOREIGN_REF;
  if (node instanceof HastChildStub) return node._refs === refs ? node._id : FOREIGN_REF;
  const id = refs.get(node);
  if (id !== undefined) return id;
  const d = Object.getOwnPropertyDescriptor(node, "_nodeId");
  if (d !== undefined && !d.enumerable) return FOREIGN_REF;
  return crossPipelineForeign(node);
}

const requireNid = makeRequireNid(nid);

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

/** `wrapNode` allowlist: an unlisted type fails loud instead of silently
 *  mis-wrapping. */
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
  throw new Error(
    `wrapNode: "${String(type)}" nodes cannot hold children, so they cannot wrap a node. ` +
      'Wrap in an element instead, e.g. { type: "element", tagName: "div", properties: {}, children: [] } ' +
      'or { raw: "<div></div>" }.',
  );
}

function hastReusedId(node: unknown, refs: NodeRefs): number | undefined {
  if (node === null || typeof node !== "object") return undefined;
  const id = nid(node as HastNode, refs);
  return id !== undefined && id !== FOREIGN_REF ? id : undefined;
}

/** Emit a set-children command in place: a root-wrapped child list, the shape
 *  `Patch::SetChildren` splices in. Reused children become refs. */
function emitHastChildrenCommand(
  buffer: CommandBuffer,
  id: number,
  children: unknown,
  refs: NodeRefs,
): boolean {
  if (!Array.isArray(children)) return false;
  return buffer.emitOpstreamCommand(CMD_SET_CHILDREN, id, () => {
    buffer.open(HAST_ROOT);
    for (const c of children) {
      if (!emitHastOp(buffer, c, false, refs)) return false;
    }
    buffer.close();
    return true;
  });
}

/** Encode `node` as the `op` structural command, emitting the op-stream
 *  payload directly into the command buffer (no intermediate copy). HAST
 *  content is always a declarative node (no raw escape hatch), so it
 *  compiles or it's a hard error. */
function emitHastTree(
  buffer: CommandBuffer,
  op: StructuralOp,
  id: number,
  node: HastNode,
  refs: NodeRefs,
  allowRootRef = false,
): void {
  const ok = buffer.emitOpstreamCommand(STRUCTURAL_CMD[op], id, () =>
    emitHastOp(buffer, node, true, refs, allowRootRef),
  );
  if (!ok) throw unencodableContentError(node);
}

/** Separate from the per-node encoder, which rejects a `root` payload. */
function emitHastRootReplace(buffer: CommandBuffer, root: HastContent, refs: NodeRefs): void {
  const ok = buffer.emitOpstreamCommand(STRUCTURAL_CMD.replace, ROOT_NODE_ID, () =>
    emitHastRootOp(buffer, root as unknown as Record<string, unknown>, refs),
  );
  if (!ok) throw unencodableContentError(root);
}

function emitHastRootOp(w: OpWriter, n: Record<string, unknown>, refs: NodeRefs): boolean {
  w.open(HAST_ROOT);
  if (n.data != null) w.data(n.data);
  const children = n.children;
  if (Array.isArray(children)) {
    for (const c of children) if (!emitHastOp(w, c, false, refs)) return false;
  }
  w.close();
  return true;
}

function emitHastOp(
  w: OpWriter,
  node: unknown,
  isRoot: boolean,
  refs: NodeRefs,
  allowRootRef = false,
): boolean {
  if (node === null || typeof node !== "object") return false;
  if (!isRoot || allowRootRef) {
    const id = hastReusedId(node, refs);
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
    for (const c of children) if (!emitHastOp(w, c, false, refs)) return false;
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
  readonly #commandBuffer: CommandBuffer = acquireCommandBuffer();
  readonly #diagnostics: HastDiagnostic[];
  /** Track accumulated node state for multiple setProperty calls on the same node. */
  readonly #pendingNodes: Map<number, HastNode> = new Map();
  readonly #handle: HastHandle;
  readonly #getSource: () => string;
  readonly #resolver: LazyChildResolver<HastReader, HastNode>;
  readonly #refs: NodeRefs;
  /** Anchor id → existing nodes spliced there; null until a visitor reuses one. */
  #reuseEdges: Map<number, Set<number>> | null = null;
  /** One canonical object per parent id, so visitors can dedupe by identity.
   *  Null until the first `parent()` call; most passes never make one. */
  #parentsById: Map<number, HastNode> | null = null;
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
    this.#commandBuffer.removeNode(requireNid(node, "removeNode", this.#refs));
  }

  replaceNode(node: HastNode, newNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "replaceNode", this.#refs);
    if (Array.isArray(newNode)) {
      if (id === ROOT_NODE_ID && newNode.length > 1) throw rootReplacementError(newNode);
      // The last node carries the `replace` so refs back to the target still splice.
      let previous: HastContent | undefined;
      for (const n of newNode) {
        if (previous !== undefined)
          emitHastTree(this.#commandBuffer, "insertBefore", id, previous, this.#refs);
        previous = n;
      }
      if (previous === undefined) {
        // Replacing with nothing drops the node, like removeNode.
        this.removeNode(node);
      } else if (id === ROOT_NODE_ID) {
        emitHastRootReplace(this.#commandBuffer, requireRootReplacement(previous), this.#refs);
      } else {
        emitHastTree(this.#commandBuffer, "replace", id, previous, this.#refs);
      }
      // A stale queued replacement would win: setProperty folds into it, landing last.
      this.#pendingNodes.delete(id);
      return;
    }
    if (id === ROOT_NODE_ID) {
      emitHastRootReplace(this.#commandBuffer, requireRootReplacement(newNode), this.#refs);
      return;
    }
    emitHastTree(this.#commandBuffer, "replace", id, newNode, this.#refs);
    // Track the replacement so a later mdxJsx setProperty can fold into it.
    this.#pendingNodes.set(id, newNode);
  }

  /** Rejects the two reuse shapes that can't be spliced by id, at the call site rather than at the end of the compile. */
  #trackReuse(anchorId: number, content: HastContent, op: string): void {
    const targetId = hastReusedId(content, this.#refs);
    if (targetId === undefined || targetId === anchorId) return;
    for (let cur = this.#resolver.parentIdOf(anchorId); cur !== undefined; ) {
      if (cur === targetId) throw reuseAncestorError(op);
      cur = this.#resolver.parentIdOf(cur);
    }
    const edges = (this.#reuseEdges ??= new Map());
    const seen = new Set<number>([targetId]);
    const queue = [targetId];
    while (queue.length > 0) {
      const next = edges.get(queue.pop()!);
      if (next === undefined) continue;
      for (const id of next) {
        if (id === anchorId) throw reuseCycleError(op);
        if (seen.add(id)) queue.push(id);
      }
    }
    let targets = edges.get(anchorId);
    if (targets === undefined) edges.set(anchorId, (targets = new Set()));
    targets.add(targetId);
  }

  insertBefore(node: HastNode, newNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "insertBefore", this.#refs);
    for (const n of asArray(newNode)) {
      this.#trackReuse(id, n, "insertBefore");
      emitHastTree(this.#commandBuffer, "insertBefore", id, n, this.#refs, true);
    }
  }

  insertAfter(node: HastNode, newNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "insertAfter", this.#refs);
    for (const n of asArray(newNode)) {
      this.#trackReuse(id, n, "insertAfter");
      emitHastTree(this.#commandBuffer, "insertAfter", id, n, this.#refs, true);
    }
  }

  wrapNode(
    node: HastNode,
    parentNode: HastParentContent | RawHastContent | RawHtmlHastContent,
  ): void {
    const id = requireNid(node, "wrapNode", this.#refs);
    if (
      typeof (parentNode as RawHastContent).raw === "string" ||
      typeof (parentNode as RawHtmlHastContent).rawHtml === "string"
    ) {
      this.#commandBuffer.wrapNode(id, parentNode as RawHastContent | RawHtmlHastContent);
      return;
    }
    assertHastWrapParent(parentNode as HastContent);
    emitHastTree(this.#commandBuffer, "wrapNode", id, parentNode as HastContent, this.#refs);
  }

  prependChild(node: HastNode, childNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "prependChild", this.#refs);
    for (const n of asArray(childNode)) {
      this.#trackReuse(id, n, "prependChild");
      emitHastTree(this.#commandBuffer, "prependChild", id, n, this.#refs, true);
    }
  }

  appendChild(node: HastNode, childNode: HastContent | HastContent[]): void {
    const id = requireNid(node, "appendChild", this.#refs);
    for (const n of asArray(childNode)) {
      this.#trackReuse(id, n, "appendChild");
      emitHastTree(this.#commandBuffer, "appendChild", id, n, this.#refs, true);
    }
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
    const id = requireNid(node, "setProperty", this.#refs);
    if (key === "children") {
      // children is structural: set-children keeps the node and swaps only its
      // child list (reused children keep their id).
      if (!emitHastChildrenCommand(this.#commandBuffer, id, value, this.#refs)) {
        throw unencodableContentError(value);
      }
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
    return textContentHandle(this.#handle, requireNid(node, "textContent", this.#refs));
  }

  parent<N extends Exclude<HastNode, HastRoot>>(node: Readonly<N>): Readonly<HastParents>;
  parent(node: Readonly<HastNode>): Readonly<HastParents> | undefined;
  parent(node: Readonly<HastNode>): Readonly<HastParents> | undefined {
    const parentId = this.#resolver.parentIdOf(requireNid(node as HastNode, "parent", this.#refs));
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
    const id = node ? nid(node, this.#refs) : undefined;
    this.#diagnostics.push({
      message,
      nodeId: id === FOREIGN_REF ? undefined : id,
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
  visit(node: Readonly<N>, ctx: HastVisitorContext): HastNode | void | Promise<HastNode | void>;
}

type HastVisitorFn<N extends HastNode = HastNode> = (
  node: Readonly<N>,
  ctx: HastVisitorContext,
) => HastNode | void | Promise<HastNode | void>;

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

/** Memoize derived subscriptions per plugin object identity. Reused plugin
 *  definitions (the common case for non-stateful plugins) avoid the per-compile
 *  walk over METHOD_TO_TYPE plus the `rustSubs.map(...)` projection for NAPI. */
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

/** Get the (cached) Rust-side projection of `subs` that strips visitFn so it
 *  can cross NAPI. Computed once per plugin object alongside `subs`. */
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

/** Caught pre-wire so the failure names the API shape, not the internal `tagFilter` field. */
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
      // Bare function, empty filter matches all nodes of this type
      subs.push({ nodeType, tagFilter: [], visitFn: value as ResolvedSubscription["visitFn"] });
    }
  }

  const rustSubs = subs.map((s) => ({ nodeType: s.nodeType, tagFilter: s.tagFilter }));
  return { subs, rustSubs };
}

/** Visitor method name → node-type tag (method names are the subscribable AST names). */
const METHOD_TO_TYPE: Record<string, number> = Object.fromEntries(
  [...VISITOR_KEYS].map((name) => [name, NAME_TO_TYPE[name]!] as const),
);

/** Build the child-stub list for a matched node from the wire's `[child_ids]
 *  [child_types]` blocks, no arena snapshot. Stale ids are caught at
 *  materialization: the resolver's epoch check refuses a snapshot once the
 *  arena has mutated or been dropped. */
function readChildStubs(
  view: DataView,
  buf: Uint8Array,
  idsPos: number,
  typesPos: number,
  count: number,
  resolver: HastLazyChildResolver,
): HastNode[] {
  // With a hot snapshot a stub's deferral buys nothing; real nodes skip its per-field getters.
  if (resolver.hasHotSnapshot()) {
    const nodes: HastNode[] = new Array(count);
    for (let i = 0; i < count; i++) {
      nodes[i] = resolver.materializeOne(view.getUint32(idsPos + i * 4, true));
    }
    return nodes;
  }
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

type HastProperties = Record<string, string | number | boolean | (string | number)[]>;

/** Per-walk wire state; one reference per element keeps constructor stores minimal. */
interface WalkWire {
  view: DataView;
  buf: Uint8Array;
  resolver: HastLazyChildResolver;
}

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
  #wire: WalkWire;
  #propsPos: number;
  #childIdsPos: number;
  #childTypesPos: number;
  #childCount: number;

  constructor(
    tagName: string,
    nodeId: number,
    wire: WalkWire,
    propsPos: number,
    propCount: number,
    childIdsPos: number,
    childTypesPos: number,
    childCount: number,
  ) {
    this.tagName = tagName;
    this.#nodeId = nodeId;
    this.#wire = wire;
    this.#propsPos = propsPos;
    this.#childIdsPos = childIdsPos;
    this.#childTypesPos = childTypesPos;
    this.#childCount = childCount;
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

  /** @internal */
  get _refs(): NodeRefs {
    return this.#wire.resolver.refs;
  }

  static {
    WALK_PROPS_DESC = {
      enumerable: true,
      configurable: true,
      get(this: WalkElement): HastProperties {
        const w = this.#wire;
        const val = decodeWalkElementProps(w.view, w.buf, this.#propsPos);
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
        const w = this.#wire;
        const val = readChildStubs(
          w.view,
          w.buf,
          this.#childIdsPos,
          this.#childTypesPos,
          this.#childCount,
          w.resolver,
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
  wire: WalkWire,
  offset: number,
  nodeId: number,
  position: Position | undefined,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  data: Record<string, unknown> | null,
): HastNode {
  // Eager: tagName (almost always accessed by visitors)
  const tagName = readWalkElementTag(wire.view, wire.buf, offset);
  const pos = walkElementPropsAt(wire.view, offset);
  const propCount = walkElementPropCount(wire.view, pos);
  const node = new WalkElement(
    tagName,
    nodeId,
    wire,
    pos,
    propCount,
    childIdsPos,
    childTypesPos,
    childCount,
  );
  if (position !== undefined) node.position = position;
  if (data !== null) node.data = data;
  return node as unknown as HastNode;
}

function readTextFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  position: Position | undefined,
  data: Record<string, unknown> | null,
  refs: NodeRefs,
): HastNode {
  const value = readWalkHastValue(view, buf, offset, nodeType);
  const base: Record<string, unknown> = {
    type: TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`,
    value,
  };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  const node = base as unknown as HastNode;
  refs.set(node, nodeId);
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
  const { name, attributes } = readWalkMdxJsx(view, buf, offset);

  const typeName = nodeType === HAST_MDX_JSX_ELEMENT ? "mdxJsxFlowElement" : "mdxJsxTextElement";
  const base: Record<string, unknown> = { type: typeName, name, attributes };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  resolver.refs.set(base, nodeId);
  makeLazyChildren(base, view, buf, childIdsPos, childTypesPos, childCount, resolver);
  return base as unknown as HastNode;
}

function readMatchedNode(
  wire: WalkWire,
  offset: number,
  nodeId: number,
  nodeType: number,
): HastNode {
  const { view, buf, resolver } = wire;
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
      wire,
      pos,
      nodeId,
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
    return readTextFromBinary(view, buf, pos, nodeId, nodeType, position, data, resolver.refs);
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
  // Fallback: root and doctype.
  const base: Record<string, unknown> = { type: TYPE_NAMES[nodeType] ?? `unknown(${nodeType})` };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  if (nodeType === HAST_ROOT) {
    // `...root.children` has to work in a hook.
    if (childCount > 0) {
      makeLazyChildren(base, view, buf, childIdsPos, childTypesPos, childCount, resolver);
    } else {
      base.children = [];
    }
  }
  const node = base as unknown as HastNode;
  resolver.refs.set(node, nodeId);
  return node;
}

const HAST_EPOCH_CACHE = registerEpochCacheSlot(new WeakMap<AnyHandle, EpochCache<HastReader>>());

class HastLazyChildResolver extends LazyChildResolver<HastReader, HastNode> {
  protected override cacheSlot() {
    return HAST_EPOCH_CACHE;
  }

  protected override createReader(wire: Uint8Array): HastReader {
    return new HastReader(wire);
  }

  protected override materializeNode(reader: HastReader, nodeId: number, refs: NodeRefs): HastNode {
    return materializeHastNode(reader, nodeId, true, refs);
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
 *  mutations (e.g. setProperty) are not clobbered.
 *
 *  A same-type text-like node carrying only a new `value` becomes a
 *  setProperty("value") rather than a structural replace, which would force
 *  the arena into a full rebuild for a shape that didn't change. */
function applyHastVisitResult(
  result: HastNode | void,
  nodeId: number,
  returnBuffer: CommandBuffer,
  originalNode: HastNode,
  refs: NodeRefs,
): void {
  if (result == null) return;
  if (result === originalNode) return;
  if (isTextValueSwap(result, originalNode)) {
    returnBuffer.setProperty(nodeId, "value", (result as { value: string }).value);
    return;
  }
  emitHastTree(returnBuffer, "replace", nodeId, result, refs);
}

function handleVisitResult(
  result: HastNode | void | Promise<HastNode | void>,
  nodeId: number,
  returnBuffer: CommandBuffer,
  deferred: { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[] | null,
  originalNode: HastNode,
  refs: NodeRefs,
): { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[] | null {
  if (result instanceof Promise) {
    const list = deferred ?? [];
    list.push({ nodeId, promise: result, originalNode });
    return list;
  }
  applyHastVisitResult(result, nodeId, returnBuffer, originalNode, refs);
  return deferred;
}

/** True when `result` is a same-type text-like node carrying only `type` +
 *  `value`. The explicit `=== undefined` checks avoid the array alloc of
 *  `Object.keys().length` on this per-text-node hot path. */
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
  returnBuffer: CommandBuffer,
  resolver: HastLazyChildResolver,
): { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[] | null {
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = matchView.getUint32(0, true);
  const wire: WalkWire = { view: matchView, buf: matchBuf, resolver };
  let deferred:
    | { nodeId: number; promise: Promise<HastNode | void>; originalNode: HastNode }[]
    | null = null;

  for (let i = 0; i < matchCount; i++) {
    const indexBase = 4 + i * 10;
    const nodeId = matchView.getUint32(indexBase, true);
    const subIndex = matchBuf[indexBase + 4]!;
    const dataOffset = matchView.getUint32(indexBase + 6, true);

    const sub = subs[subIndex]!;
    const node = readMatchedNode(wire, dataOffset, nodeId, sub.nodeType);
    const result = sub.visitFn(node, ctx);
    deferred = handleVisitResult(result, nodeId, returnBuffer, deferred, node, resolver.refs);
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
    return result.then((commands) => applyCollectedCommands(handle, commands));
  }
  return applyCollectedCommands(handle, result);
}

/** Apply commands collected by `visitHastHandleCollect`; returns the number of
 *  patches dropped as stranded (0 when none). */
function applyCollectedCommands(handle: HastHandle, commands: Uint8Array): number {
  if (commands.length === 0) return 0;
  markHandleMutated(handle);
  return applyCommandsToHandle(handle, commands);
}

/** Run a HAST visitor, build the command buffer, but do NOT apply it. Returns
 *  the merged commands so the caller can choose how to dispatch: either via
 *  `applyCommandsToHandle` (intermediate plugins in a chain) or via a fused
 *  NAPI call like `applyCommandsAndRenderHandle` (final plugin, saves one
 *  apply + one render + one drop crossing). Empty result means no mutations.
 */
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
  const returnBuffer = acquireCommandBuffer();
  const rustSubs = getRustSubs(plugin);
  const deferred = dispatchMatches(walkHandle(handle, rustSubs), subs, ctx, returnBuffer, resolver);

  if (deferred) {
    return Promise.all(
      deferred.map((d) =>
        d.promise.then((result) => ({ nodeId: d.nodeId, result, originalNode: d.originalNode })),
      ),
    ).then((results) => {
      for (const { nodeId, result, originalNode } of results) {
        applyHastVisitResult(result, nodeId, returnBuffer, originalNode, resolver.refs);
      }
      return collectCommands(returnBuffer, ctx);
    });
  }

  return collectCommands(returnBuffer, ctx);
}

const HAST_ROOT_SUBS: { nodeType: number; tagFilter: string[] }[] = [
  { nodeType: HAST_ROOT, tagFilter: [] },
];

/** Its own pass so the caller can apply what `before` queued before the
 *  visitors walk, and the visitors' mutations before `after` reads the tree. */
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
  const returnBuffer = acquireCommandBuffer();
  const matchBuf = walkHandle(handle, HAST_ROOT_SUBS);
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  if (matchView.getUint32(0, true) === 0) return collectCommands(returnBuffer, ctx);

  const wire: WalkWire = { view: matchView, buf: matchBuf, resolver };
  const root = readMatchedNode(
    wire,
    matchView.getUint32(10, true),
    matchView.getUint32(4, true),
    HAST_ROOT,
  ) as HastRoot;

  const result = hook.call(plugin, root, ctx);
  if (result instanceof Promise) return result.then(() => collectCommands(returnBuffer, ctx));
  return collectCommands(returnBuffer, ctx);
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
    return result.then((commands) => applyCollectedCommands(handle, commands));
  }
  return applyCollectedCommands(handle, result);
}

function collectCommands(returnBuffer: CommandBuffer, ctx: HastVisitorContextImpl): Uint8Array {
  const { merged } = mergeAndReset(returnBuffer, ctx);
  // Return the buffers to the pool. The merged Uint8Array above already
  // copied the bytes out, so the underlying ArrayBuffers can be reused.
  releaseCommandBuffer(returnBuffer);
  releaseCommandBuffer(ctx.getCommandBuffer());
  return merged;
}
