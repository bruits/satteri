import { mdastTextContentHandle, nodeLabels, walkMdastHandle } from "#binding";
import type {
  Blockquote,
  Break,
  Code,
  Definition,
  Delete,
  Emphasis,
  FootnoteDefinition,
  FootnoteReference,
  Heading,
  Html,
  Image,
  ImageReference,
  InlineCode,
  Link,
  LinkReference,
  List,
  ListItem,
  Parents as MdastParents,
  Root as MdastRoot,
  Paragraph,
  Strong,
  Table,
  TableCell,
  TableRow,
  Text,
  ThematicBreak,
  Yaml,
} from "mdast";
import {
  acquireCommandBuffer,
  classifyReturn,
  CommandBuffer,
  STRUCTURAL_CMD,
  type StructuralOp,
} from "../command-buffer.js";
import type { ContainerDirective, LeafDirective, TextDirective } from "../directive-types.js";
import { CMD_SET_CHILDREN } from "../generated/wire-constants.js";
import type { MdastHandle } from "../handles.js";
import { LazyChildResolver } from "../lazy-child-resolver.js";
import type {
  MdxFlowExpression,
  MdxjsEsm,
  MdxJsxFlowElement,
  MdxJsxTextElement,
  MdxTextExpression,
} from "../mdx-types.js";
import {
  directiveAttributesJson,
  emitMdxAttr,
  mdxAttributesJson,
  OF_ALT,
  OF_CHECKED,
  OF_DEPTH,
  OF_EXPLICIT,
  OF_IDENTIFIER,
  OF_LABEL,
  OF_LANG,
  OF_META,
  OF_NAME,
  OF_ORDERED,
  OF_REFERENCE_TYPE,
  OF_SPREAD,
  OF_START,
  OF_TITLE,
  OF_URL,
  OF_VALUE,
  OpWriter,
  PROP_STRING,
} from "../op-stream.js";
import type {
  Custom,
  Data,
  DescriptionDetails,
  DescriptionList,
  DescriptionTerm,
  InlineMath,
  MathNode,
  MdastNode,
  SourceFormat,
  Subscript,
  Superscript,
  Toml,
} from "../types.js";
import {
  asArray,
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
  requireDirectiveAttributeValue,
  requireHeadingDepth,
  requireMdxAttributeValue,
  unbuildableNodeError,
  usageError,
} from "../usage-errors.js";
import { ru32 } from "../wire-read.js";
import { MDAST_OPSTREAM_TYPES, NAME_TO_TYPE, VISITOR_KEYS } from "./generated/node-types.js";
import { LEAF_TYPES } from "./mdast-materializer.js";
import { MdastReader } from "./mdast-reader.js";
import { getNodeId, MdastLazyChildResolver, readMdastMatchedNode } from "./mdast-walk.js";

/** A string spliced into the tree, re-parsed as Markdown. Set `mdxExpressions:
 *  false` to keep MDX `{…}` literal, needed when injecting generated HTML
 *  (KaTeX, highlighters, diagrams) whose braces aren't expressions. Default true. */
export interface RawMdastContent {
  raw: string;
  mdxExpressions?: boolean;
}

export interface RawHtmlMdastContent {
  /** @deprecated Use the equivalent `{ raw, mdxExpressions: false }`. */
  rawHtml: string;
}

/** New content for a structural mutation: a declarative node, or a raw string
 *  escape hatch ({@link RawMdastContent}). Declarative nodes compile to the
 *  op-stream; a type the op-stream can't encode is a hard error. */
export type MdastContent = MdastNode | Custom | RawMdastContent | RawHtmlMdastContent;

/** An existing node a mutation targets. Includes {@link Custom} so a node
 *  reached through the `custom` visitor can be passed straight back in. */
export type MdastTarget = MdastNode | Custom;

/** A `wrapNode` wrapper: a built-in parent, or a user-defined node declaring
 *  a children array. */
export type MdastParentContent =
  | Exclude<Extract<MdastNode, { children: unknown[] }>, MdastRoot>
  | (Custom & { children: NonNullable<Custom["children"]> });

export interface MdastDiagnostic {
  message: string;
  nodeId?: number | undefined;
  position?: MdastNode["position"] | undefined;
  severity: "error" | "warning" | "info";
}

export interface MdastVisitorContext {
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
  removeNode(node: Readonly<MdastTarget>): void;
  /**
   * Swap `node` for one node, or for an array of nodes placed in order at its
   * position. An empty array drops the node, the same as `removeNode`.
   * The document root takes a `root`, the one place a `root` is accepted as
   * content, or a raw string, which parses to a root of its own.
   */
  replaceNode(node: Readonly<MdastTarget>, newNode: MdastContent | MdastContent[]): void;
  insertBefore(node: Readonly<MdastTarget>, newNode: MdastContent | MdastContent[]): void;
  insertAfter(node: Readonly<MdastTarget>, newNode: MdastContent | MdastContent[]): void;
  /**
   * Wrap `node` in `parentNode`, making it `parentNode`'s first child. Any
   * children `parentNode` declares are kept after it. `parentNode` must be a
   * node type that can hold children, or a raw string parsing to exactly one
   * such block (`{ raw: "> " }` wraps in a blockquote); to surround a node
   * with raw HTML tags, use `replaceNode(node, [openTag, node, closeTag])`
   * instead.
   */
  wrapNode(
    node: Readonly<MdastTarget>,
    parentNode: MdastParentContent | RawMdastContent | RawHtmlMdastContent,
  ): void;
  prependChild(node: Readonly<MdastTarget>, childNode: MdastContent | MdastContent[]): void;
  appendChild(node: Readonly<MdastTarget>, childNode: MdastContent | MdastContent[]): void;
  /** Insert one node or an array at `index`; clamps (`0` or less prepends, past the end appends). */
  insertChildAt(
    node: Readonly<MdastTarget>,
    index: number,
    childNode: MdastContent | MdastContent[],
  ): void;
  /** Remove the `index`-th child of `node`; a no-op when there is no such child. */
  removeChildAt(node: Readonly<MdastTarget>, index: number): void;
  setField<N extends MdastTarget, K extends SettableFieldKey<N, true>>(
    node: Readonly<N>,
    key: K,
    value: Exclude<N[K], undefined>,
  ): void;
  /** Table alignment is an array stored in the table's typed data. */
  setField(node: Readonly<Table>, key: "align", value: Table["align"]): void;
  /** `children` is structural and every parent accepts it, including node-type unions. */
  setField(node: Readonly<MdastTarget>, key: "children", value: readonly MdastTarget[]): void;
  /** `data` is an open per-node bag serialized to JSON. `null` clears it. */
  setField(node: Readonly<MdastTarget>, key: "data", value: Record<string, unknown> | null): void;
  /** Set one string entry in a directive's `attributes`. */
  setAttribute(
    node: Readonly<ContainerDirective | LeafDirective | TextDirective>,
    name: string,
    value: string,
  ): void;
  /** Set one entry in an MDX JSX element's `attributes`. */
  setAttribute(
    node: Readonly<MdxJsxFlowElement | MdxJsxTextElement>,
    name: string,
    value: unknown,
  ): void;
  /** @deprecated MDAST has no property container; use `setField`. */
  setProperty<N extends MdastTarget, K extends keyof N & string>(
    node: Readonly<N>,
    key: K,
    value: N[K],
  ): void;
  /** `children` is structural and every parent accepts it, so the key also
   *  works on node-type unions (e.g. a node returned by `parent()`). */
  setProperty(node: Readonly<MdastTarget>, key: "children", value: readonly MdastTarget[]): void;
  /** `data` is an open per-node bag serialized to JSON on the wire, so it
   *  accepts any record (hName/hProperties/custom fields), not just the node's
   *  declared `data` shape. `null` clears it. */
  setProperty(
    node: Readonly<MdastTarget>,
    key: "data",
    value: Record<string, unknown> | null,
  ): void;
  /** Collect the concatenated text of all descendant text nodes (like mdast-util-to-string). */
  textContent(
    node: Readonly<MdastTarget>,
    options?: { includeImageAlt?: boolean; includeHtml?: boolean },
  ): string;
  /**
   * The parent of a node, or `undefined` at the root. Within a pass the same
   * parent is always the same object, so visitors on sibling nodes can dedupe
   * by identity.
   */
  parent<N extends Exclude<MdastNode, MdastRoot>>(node: Readonly<N>): Readonly<MdastParents>;
  parent(node: Readonly<MdastTarget>): Readonly<MdastParents> | undefined;
  /**
   * Index of `node` within its parent's children, or `undefined` at the root.
   * Use this rather than `parent.children.indexOf(node)`, which won't find it.
   */
  indexOf(node: Readonly<MdastTarget>): number | undefined;
  report(opts: {
    message: string;
    node?: Readonly<MdastTarget>;
    severity?: "error" | "warning" | "info";
  }): void;
  getDiagnostics(): MdastDiagnostic[];
}

const requireNid = makeRequireNid(getNodeId);

class MdastVisitorContextImpl implements MdastVisitorContext {
  readonly #commandBuffer: CommandBuffer = acquireCommandBuffer();
  readonly #diagnostics: MdastDiagnostic[];
  /** Value nodes edited this pass, whose own return can no longer be sent as a value-only edit. */
  #editedValueNodes: Set<number> | undefined;
  readonly #handle: MdastHandle;
  readonly #getSource: () => string;
  readonly #resolver: LazyChildResolver<MdastReader, MdastNode>;
  readonly #refs: NodeRefs;
  readonly removals: RemovalTracker<MdastNode>;
  readonly fileURL: URL | undefined;
  readonly data: Data;
  readonly sourceFormat: SourceFormat;

  constructor(
    handle: MdastHandle,
    getSource: () => string,
    fileURL: URL | undefined,
    resolver: LazyChildResolver<MdastReader, MdastNode>,
    data: Data,
    sourceFormat: SourceFormat,
    diagnostics: MdastDiagnostic[],
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

  removeNode(node: Readonly<MdastTarget>): void {
    const id = requireNid(node as MdastNode, "removeNode", this.#refs);
    this.#commandBuffer.removeNode(id);
    this.removals.removed(id);
  }

  insertBefore(node: Readonly<MdastTarget>, newNode: MdastContent | MdastContent[]): void {
    this.#splice(node, newNode, "insertBefore", "insertBefore");
  }

  insertAfter(node: Readonly<MdastTarget>, newNode: MdastContent | MdastContent[]): void {
    this.#splice(node, newNode, "insertAfter", "insertAfter");
  }

  #splice(
    anchor: Readonly<MdastTarget>,
    content: MdastContent | MdastContent[],
    op: StructuralOp,
    method: string,
  ): void {
    const anchorId = requireNid(anchor as MdastNode, method, this.#refs);
    if (op === "insertBefore" || op === "insertAfter") {
      requireSiblingAnchor(method, anchorId === ROOT_NODE_ID);
    }
    requireContent(method, content);
    const nodes = asArray(content);
    for (const n of nodes) {
      if (getNodeId(n as MdastNode, this.#refs) === anchorId) throw selfInsertError(method);
    }
    this.removals.inserted(anchorId, content);
    if (nodes.length === 1) {
      emitMdastTree(this.#commandBuffer, op, anchorId, nodes[0]!, false, this.#refs, true);
    } else if (nodes.length > 1) {
      emitMdastSequence(
        this.#commandBuffer,
        STRUCTURAL_CMD[op],
        anchorId,
        nodes,
        false,
        this.#refs,
      );
    }
  }

  wrapNode(
    node: Readonly<MdastTarget>,
    parentNode: MdastParentContent | RawMdastContent | RawHtmlMdastContent,
  ): void {
    const id = requireNid(node as MdastNode, "wrapNode", this.#refs);
    requireWrapper(parentNode);
    assertMdastWrapParent(parentNode);
    const wrapperId = getNodeId(parentNode as MdastNode, this.#refs);
    if (wrapperId === FOREIGN_REF) throw foreignContentError();
    if (wrapperId !== undefined) throw existingWrapperError();
    this.removals.inserted(id, parentNode);
    emitMdastTree(this.#commandBuffer, "wrapNode", id, parentNode, false, this.#refs);
  }

  prependChild(node: Readonly<MdastTarget>, childNode: MdastContent | MdastContent[]): void {
    this.#splice(node, childNode, "prependChild", "prependChild");
  }

  appendChild(node: Readonly<MdastTarget>, childNode: MdastContent | MdastContent[]): void {
    this.#splice(node, childNode, "appendChild", "appendChild");
  }

  insertChildAt(
    node: Readonly<MdastTarget>,
    index: number,
    childNode: MdastContent | MdastContent[],
  ): void {
    const children = ("children" in node ? node.children : undefined) ?? [];
    const [anchor, op] =
      index <= 0 || children.length === 0
        ? ([node, "prependChild"] as const)
        : index >= children.length
          ? ([node, "appendChild"] as const)
          : ([children[index]!, "insertBefore"] as const);
    this.#splice(anchor, childNode, op, "insertChildAt");
  }

  removeChildAt(node: Readonly<MdastTarget>, index: number): void {
    const child = "children" in node ? node.children?.[index] : undefined;
    if (child) this.removeNode(child);
  }

  replaceNode(node: Readonly<MdastTarget>, newNode: MdastContent | MdastContent[]): void {
    const id = requireNid(node as MdastNode, "replaceNode", this.#refs);
    requireContent("replaceNode", newNode);
    const onlyReplacement = Array.isArray(newNode)
      ? newNode.length === 1
        ? newNode[0]
        : undefined
      : newNode;
    if (
      id === ROOT_NODE_ID &&
      onlyReplacement !== undefined &&
      reusedId(onlyReplacement, this.#refs) === id
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
      emitMdastSequence(this.#commandBuffer, STRUCTURAL_CMD.replace, id, nodes, true, this.#refs);
      return;
    }
    if (id === ROOT_NODE_ID && !isRawMdastContent(onlyReplacement)) {
      emitMdastRootReplace(
        this.#commandBuffer,
        requireRootReplacement(onlyReplacement),
        this.#refs,
      );
    } else {
      emitMdastTree(this.#commandBuffer, "replace", id, onlyReplacement, true, this.#refs, true);
    }
    this.#noteEdit(node, id);
  }

  #noteEdit(node: Readonly<MdastTarget>, id: number): void {
    if (MDAST_VALUE_ONLY_TYPES.has(node.type)) (this.#editedValueNodes ??= new Set()).add(id);
  }

  /** A visitor's return is its last edit, queued after its context calls. */
  applyReturn(result: MdastVisitorResult, nodeId: number, originalNode: MdastNode): void {
    // Returning the input node must not overwrite mutations queued through the context.
    if (result === undefined || result === null || result === originalNode) return;
    requireVisitorResult(originalNode, result, true);
    const buffer = this.#commandBuffer;
    if (Array.isArray(result)) {
      if (result.length === 1) {
        this.applyReturn(result[0], nodeId, originalNode);
        return;
      }
      this.removals.returned(nodeId, result);
      if (result.length === 0) buffer.removeNode(nodeId);
      else emitMdastSequence(buffer, STRUCTURAL_CMD.replace, nodeId, result, true, this.#refs);
      return;
    }
    switch (classifyReturn(result)) {
      case "raw_markdown":
        buffer.replace(nodeId, result as unknown as RawMdastContent);
        break;
      case "raw_html":
        buffer.replace(nodeId, result as unknown as RawHtmlMdastContent);
        break;
      case "structured_node": {
        const node = result as MdastNode;
        // A value-only edit stands in for the replacement only while nothing else has edited the node.
        if (isMdastTextValueSwap(node, originalNode) && !this.#editedValueNodes?.has(nodeId)) {
          buffer.setProperty(nodeId, "value", node.value);
          break;
        }
        this.removals.rebuilt(nodeId, node);
        emitMdastTree(buffer, "replace", nodeId, node as MdastContent, true, this.#refs, true);
        break;
      }
    }
  }

  setField(node: Readonly<MdastTarget>, key: string, value: unknown): void {
    const id = requireNid(node as MdastNode, "setField", this.#refs);
    requireSettableField("setField", key);
    this.#noteEdit(node, id);
    if (key === "children") {
      requireChildren(value);
      this.removals.rebuilt(id, value);
      emitMdastChildrenCommand(this.#commandBuffer, id, value as readonly unknown[], this.#refs);
      return;
    }
    if (key === "data") value = value != null ? dataJson(value) : null;
    else if (key === "attributes") {
      value = isDirective(node) ? directiveAttributesJson(value) : mdxAttributesJson(value);
    } else if (key === "align") value = JSON.stringify(alignCodes(value));
    else if (key === "depth" && node.type === "heading") requireHeadingDepth(value);
    this.#commandBuffer.setField(id, key, value);
  }

  setAttribute(node: Readonly<MdastTarget>, name: string, value: unknown): void {
    const id = requireNid(node as MdastNode, "setAttribute", this.#refs);
    if (isDirective(node)) requireDirectiveAttributeValue(name, value, false);
    else if (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") {
      requireMdxAttributeValue(name, value);
    } else {
      throw usageError(
        `setAttribute: ${nodeOfType(node.type)} has no attributes; only directives and MDX JSX elements do`,
        TypeError,
      );
    }
    this.#commandBuffer.setAttribute(id, name, value);
  }

  setProperty(node: Readonly<MdastTarget>, key: string, value: unknown): void {
    requireNid(node as MdastNode, "setProperty", this.#refs);
    this.setField(node, key, value);
  }

  textContent(
    node: Readonly<MdastTarget>,
    options?: { includeImageAlt?: boolean; includeHtml?: boolean },
  ): string {
    return mdastTextContentHandle(
      this.#handle,
      requireNid(node as MdastNode, "textContent", this.#refs),
      options,
    );
  }

  parent<N extends Exclude<MdastNode, MdastRoot>>(node: Readonly<N>): Readonly<MdastParents>;
  parent(node: Readonly<MdastTarget>): Readonly<MdastParents> | undefined;
  parent(node: Readonly<MdastTarget>): Readonly<MdastParents> | undefined {
    const parentId = this.#resolver.parentIdOf(requireNid(node as MdastNode, "parent", this.#refs));
    if (parentId === undefined) return undefined;
    return this.#resolver.materializeOne(parentId) as MdastParents;
  }

  indexOf(node: Readonly<MdastTarget>): number | undefined {
    return this.#resolver.indexInParent(requireNid(node as MdastNode, "indexOf", this.#refs));
  }

  report({
    message,
    node,
    severity = "error",
  }: {
    message: string;
    node?: Readonly<MdastTarget>;
    severity?: "error" | "warning" | "info";
  }): void {
    const id = node ? getNodeId(node as MdastNode, this.#refs) : undefined;
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

  getDiagnostics(): MdastDiagnostic[] {
    return this.#diagnostics;
  }
}

type MdastVisitorResult = MdastContent | MdastContent[] | undefined | null | void;

export type MdastVisitorFn<N extends MdastNode | Custom = MdastNode> = (
  node: Readonly<N>,
  context: MdastVisitorContext,
) => MdastVisitorResult | Promise<MdastVisitorResult>;

export type MdastHookFn = (
  root: Readonly<MdastRoot>,
  context: MdastVisitorContext,
) => void | Promise<void>;

export interface MdastPluginInstance {
  /** Plugin-level configuration (e.g. `{ position: true }` to read positions). */
  options?: PluginOptions;
  /** Runs once per document, before the plugin's visitors. Awaited when async. */
  before?: MdastHookFn;
  /** Runs once per document, after the plugin's visitors have settled. Awaited
   *  when async. */
  after?: MdastHookFn;
  paragraph?: MdastVisitorFn<Paragraph>;
  heading?: MdastVisitorFn<Heading>;
  thematicBreak?: MdastVisitorFn<ThematicBreak>;
  blockquote?: MdastVisitorFn<Blockquote>;
  list?: MdastVisitorFn<List>;
  listItem?: MdastVisitorFn<ListItem>;
  html?: MdastVisitorFn<Html>;
  code?: MdastVisitorFn<Code>;
  definition?: MdastVisitorFn<Definition>;
  text?: MdastVisitorFn<Text>;
  emphasis?: MdastVisitorFn<Emphasis>;
  strong?: MdastVisitorFn<Strong>;
  inlineCode?: MdastVisitorFn<InlineCode>;
  break?: MdastVisitorFn<Break>;
  link?: MdastVisitorFn<Link>;
  image?: MdastVisitorFn<Image>;
  linkReference?: MdastVisitorFn<LinkReference>;
  imageReference?: MdastVisitorFn<ImageReference>;
  footnoteDefinition?: MdastVisitorFn<FootnoteDefinition>;
  footnoteReference?: MdastVisitorFn<FootnoteReference>;
  table?: MdastVisitorFn<Table>;
  tableRow?: MdastVisitorFn<TableRow>;
  tableCell?: MdastVisitorFn<TableCell>;
  delete?: MdastVisitorFn<Delete>;
  yaml?: MdastVisitorFn<Yaml>;
  toml?: MdastVisitorFn<Toml>;
  math?: MdastVisitorFn<MathNode>;
  inlineMath?: MdastVisitorFn<InlineMath>;
  containerDirective?: MdastVisitorFn<ContainerDirective>;
  leafDirective?: MdastVisitorFn<LeafDirective>;
  textDirective?: MdastVisitorFn<TextDirective>;
  superscript?: MdastVisitorFn<Superscript>;
  subscript?: MdastVisitorFn<Subscript>;
  descriptionList?: MdastVisitorFn<DescriptionList>;
  descriptionTerm?: MdastVisitorFn<DescriptionTerm>;
  descriptionDetails?: MdastVisitorFn<DescriptionDetails>;
  mdxJsxFlowElement?: MdastVisitorFn<MdxJsxFlowElement>;
  mdxJsxTextElement?: MdastVisitorFn<MdxJsxTextElement>;
  mdxFlowExpression?: MdastVisitorFn<MdxFlowExpression>;
  mdxTextExpression?: MdastVisitorFn<MdxTextExpression>;
  mdxjsEsm?: MdastVisitorFn<MdxjsEsm>;
  /** Fires for every user-defined node (any node created with a `type` outside
   *  the built-in set). Discriminate with `node.type`. */
  custom?: MdastVisitorFn<Custom>;
}

interface MdastVisitResult {
  /** Binary command buffer containing all mutations. */
  commandBuffer: Uint8Array;
  diagnostics: MdastDiagnostic[];
  hasMutations: boolean;
}

export type { MdastHandle };

interface MdastSubscription {
  nodeType: number;
  visitFn: MdastVisitorFn;
}

// Cache subscriptions by plugin identity to avoid rebuilding them on every compile.
type CachedMdastSubs = {
  subs: MdastSubscription[];
  rustSubs: { nodeType: number; tagFilter: string[] }[];
};
const mdastSubscriptionCache: WeakMap<MdastPluginInstance, CachedMdastSubs> = new WeakMap();

export function resolveMdastSubscriptions(plugin: MdastPluginInstance): MdastSubscription[] {
  const cached = mdastSubscriptionCache.get(plugin);
  if (cached !== undefined) return cached.subs;
  const built = buildMdastSubscriptions(plugin);
  mdastSubscriptionCache.set(plugin, built);
  return built.subs;
}

function getMdastRustSubs(
  plugin: MdastPluginInstance,
): { nodeType: number; tagFilter: string[] }[] {
  const cached = mdastSubscriptionCache.get(plugin);
  if (cached !== undefined) return cached.rustSubs;
  const built = buildMdastSubscriptions(plugin);
  mdastSubscriptionCache.set(plugin, built);
  return built.rustSubs;
}

function buildMdastSubscriptions(plugin: MdastPluginInstance): CachedMdastSubs {
  const subs: MdastSubscription[] = [];
  for (const [name, fn] of Object.entries(plugin)) {
    if (VISITOR_KEYS.has(name) && typeof fn === "function") {
      const nodeType = NAME_TO_TYPE[name];
      if (nodeType !== undefined) {
        subs.push({
          nodeType,
          visitFn: fn as MdastSubscription["visitFn"],
        });
      }
    }
  }
  const rustSubs = subs.map((s) => ({ nodeType: s.nodeType, tagFilter: [] as string[] }));
  return { subs, rustSubs };
}

const MDAST_ROOT = NAME_TO_TYPE.root!;
const MDAST_CUSTOM = NAME_TO_TYPE.custom!;

function reusedId(node: unknown, refs: NodeRefs): number | undefined {
  if (node === null || typeof node !== "object") return undefined;
  const id = getNodeId(node as MdastNode, refs);
  return id !== undefined && id !== FOREIGN_REF ? id : undefined;
}

function emitMdastChildrenCommand(
  buffer: CommandBuffer,
  id: number,
  children: readonly unknown[],
  refs: NodeRefs,
): void {
  buffer.emitOpstreamCommand(CMD_SET_CHILDREN, id, () => {
    buffer.open(MDAST_ROOT);
    for (const c of children) emitMdastOp(buffer, c, false, false, refs);
    buffer.close();
  });
}

/** True for content the op-stream can carry inside one root-wrapped payload. */
function isPlainReplacement(content: MdastContent): boolean {
  return (
    !isRawMdastContent(content) && (content as { _keepChildren?: unknown })._keepChildren !== true
  );
}

/** Several nodes as one edit, root-wrapped so the engine splices them in at the target. */
function emitMdastSequence(
  buffer: CommandBuffer,
  cmd: number,
  id: number,
  nodes: readonly MdastContent[],
  forReplace: boolean,
  refs: NodeRefs,
): void {
  if (nodes.every(isPlainReplacement)) {
    buffer.emitOpstreamCommand(cmd, id, () => {
      buffer.open(MDAST_ROOT);
      for (const n of nodes) emitMdastOp(buffer, n, false, forReplace, refs);
      buffer.close();
    });
  } else {
    buffer.emitSequenceCommand(cmd, id, nodes, (n) =>
      emitMdastOp(buffer, n, true, forReplace, refs, true),
    );
  }
}

// Root replacement needs a separate encoder because per-node encoding rejects root payloads.
function emitMdastRootReplace(buffer: CommandBuffer, root: MdastContent, refs: NodeRefs): void {
  buffer.emitOpstreamCommand(STRUCTURAL_CMD.replace, ROOT_NODE_ID, () =>
    emitMdastRootOp(buffer, root as unknown as Record<string, unknown>, refs),
  );
}

function emitMdastRootOp(w: OpWriter, n: Record<string, unknown>, refs: NodeRefs): void {
  w.open(MDAST_ROOT);
  if (n.data != null) w.data(n.data);
  if (n._keepChildren === true) {
    w.keepChildren();
  } else {
    const children = n.children;
    if (Array.isArray(children)) {
      for (const c of children) emitMdastOp(w, c, false, true, refs);
    }
  }
  w.close();
}

function emitMdastOp(
  w: OpWriter,
  node: unknown,
  isRoot: boolean,
  forReplace: boolean,
  refs: NodeRefs,
  allowRootRef = false,
): void {
  if (node === null || typeof node !== "object" || Array.isArray(node)) {
    throw unbuildableNodeError(node, false);
  }
  const id = getNodeId(node as MdastNode, refs);
  if (id === FOREIGN_REF) throw foreignContentError();
  if ((!isRoot || allowRootRef) && id !== undefined) {
    w.ref(id);
    return;
  }
  const n = node as Record<string, unknown>;
  let type = MDAST_OPSTREAM_TYPES[n.type as string];
  let isCustom = false;
  if (type === undefined) {
    // Unsupported built-in types must fail rather than silently becoming custom nodes.
    if (typeof n.type !== "string" || n.type.length === 0 || NAME_TO_TYPE[n.type] !== undefined) {
      throw unbuildableNodeError(n, true);
    }
    type = MDAST_CUSTOM;
    isCustom = true;
  } else if (type === MDAST_CUSTOM) {
    isCustom = true;
  }
  w.open(type);
  w.position(n.position);
  if (isCustom) w.str(OF_NAME, n.type as string);
  if (typeof n.value === "string") w.str(OF_VALUE, n.value);
  if (typeof n.url === "string") w.str(OF_URL, n.url);
  if (typeof n.title === "string") w.str(OF_TITLE, n.title);
  if (typeof n.alt === "string") w.str(OF_ALT, n.alt);
  if (typeof n.lang === "string") w.str(OF_LANG, n.lang);
  if (typeof n.meta === "string") w.str(OF_META, n.meta);
  if (typeof n.identifier === "string") w.str(OF_IDENTIFIER, n.identifier);
  if (typeof n.label === "string") w.str(OF_LABEL, n.label);
  if (typeof n.referenceType === "string") w.str(OF_REFERENCE_TYPE, n.referenceType);
  // Reject out-of-range values before integer writes can truncate them.
  if (n.depth !== undefined) {
    requireHeadingDepth(n.depth);
    w.u8(OF_DEPTH, n.depth as number);
  }
  if (typeof n.checked === "boolean") w.u8(OF_CHECKED, n.checked ? 1 : 0);
  if (typeof n.start === "number") {
    if (!Number.isInteger(n.start) || n.start < 0 || n.start > 4294967295) {
      throw usageError(
        `a list's "start" is a whole number from 0 to 4294967295, not ${describeValue(n.start)}`,
        TypeError,
      );
    }
    w.u32(OF_START, n.start);
  }
  if (typeof n.ordered === "boolean") w.bool(OF_ORDERED, n.ordered);
  if (typeof n.spread === "boolean") w.bool(OF_SPREAD, n.spread);
  if (!isCustom && typeof n.name === "string") w.str(OF_NAME, n.name);
  const attrs = n.attributes;
  if (Array.isArray(attrs)) {
    for (const a of attrs) emitMdxAttr(w, a as Record<string, unknown>);
  } else if (attrs !== null && typeof attrs === "object") {
    for (const key in attrs as Record<string, unknown>) {
      const v = (attrs as Record<string, unknown>)[key];
      requireDirectiveAttributeValue(key, v, true);
      if (typeof v === "string") w.prop(key, PROP_STRING, v);
    }
  }
  if (n.align !== undefined) w.align(alignCodes(n.align));
  if ((n.data as Record<string, unknown> | null | undefined)?._mdxExplicitJsx === true) {
    w.bool(OF_EXPLICIT, true);
  }
  if (n.data != null) w.data(n.data);
  if (isRoot && forReplace && n._keepChildren === true) {
    w.keepChildren();
  } else {
    // Only replacement can reuse the target’s original children.
    const children = n.children;
    if (Array.isArray(children)) {
      for (const c of children) emitMdastOp(w, c, false, forReplace, refs);
    }
  }
  w.close();
}

const ALIGN_CODES: Record<string, number> = { left: 1, right: 2, center: 3 };

/** A table's `align`, one entry per column: "left", "right", "center" or null. */
function alignCodes(align: unknown): number[] {
  if (align === null || align === undefined) return [];
  if (!Array.isArray(align)) {
    throw usageError(`a table's "align" is an array, not ${describeValue(align)}`, TypeError);
  }
  return align.map((a: unknown) => {
    if (a === null || a === undefined) return 0;
    const code = typeof a === "string" ? ALIGN_CODES[a] : undefined;
    if (code !== undefined) return code;
    throw usageError(
      `a table's "align" holds "left", "right", "center" or null, not ${describeValue(a)}`,
      TypeError,
    );
  });
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

function isRawMdastContent(
  content: MdastContent,
): content is RawMdastContent | RawHtmlMdastContent {
  const c = content as unknown as Record<string, unknown>;
  return typeof c.raw === "string" || typeof c.rawHtml === "string";
}

function isDirective(
  node: Readonly<MdastTarget>,
): node is ContainerDirective | LeafDirective | TextDirective {
  return (
    node.type === "containerDirective" ||
    node.type === "leafDirective" ||
    node.type === "textDirective"
  );
}

// Keep direct method calls so structural command dispatch stays monomorphic.
function emitMdastTree(
  buffer: CommandBuffer,
  op: StructuralOp,
  id: number,
  content: MdastContent,
  forReplace: boolean,
  refs: NodeRefs,
  allowRootRef = false,
): void {
  if (isRawMdastContent(content)) {
    switch (op) {
      case "replace":
        return buffer.replace(id, content);
      case "insertBefore":
        return buffer.insertBefore(id, content);
      case "insertAfter":
        return buffer.insertAfter(id, content);
      case "prependChild":
        return buffer.prependChild(id, content);
      case "appendChild":
        return buffer.appendChild(id, content);
      case "wrapNode":
        return buffer.wrapNode(id, content);
    }
  }
  buffer.emitOpstreamCommand(STRUCTURAL_CMD[op], id, () =>
    emitMdastOp(buffer, content, true, forReplace, refs, allowRootRef),
  );
}

/** A leaf wrapper would make the patch engine drop or displace the wrapped node. */
function assertMdastWrapParent(parentNode: MdastContent): void {
  const sandwich =
    'replaceNode(node, [{ type: "html", value: "<div>" }, node, { type: "html", value: "</div>" }])';
  // A raw wrapper is only a tree once Rust has parsed it, so it is checked there.
  if (isRawMdastContent(parentNode)) return;
  const type = (parentNode as { type?: unknown }).type;
  const tag = typeof type === "string" ? NAME_TO_TYPE[type] : undefined;
  if (tag === undefined) {
    if (Array.isArray((parentNode as Custom).children)) return;
    throw usageError(
      `wrapNode: a user-defined "${String(type)}" wrapper must declare a children array. ` +
        "A leaf-shaped custom node renders as text and cannot hold the wrapped node.",
    );
  }
  if (!LEAF_TYPES.has(tag)) return;
  throw usageError(
    `wrapNode: "${String(type)}" nodes cannot hold children, so they cannot wrap a node. ` +
      'Pass a parent node such as { type: "blockquote", children: [] }, a raw wrapper ' +
      `such as { raw: "> " }, or surround the node with tags: ${sandwich}.`,
  );
}

// Updating only value avoids an arena rebuild for these node types.
const MDAST_VALUE_ONLY_TYPES = new Set<string>([
  "text",
  "html",
  "inlineCode",
  "yaml",
  "toml",
  "inlineMath",
]);

// Extra fields require full replacement so the value-only fast path cannot discard them.
function isMdastTextValueSwap(
  result: MdastNode,
  original: MdastNode | undefined,
): result is MdastNode & { value: string } {
  if (original === undefined) return false;
  if (result.type !== original.type) return false;
  if (!MDAST_VALUE_ONLY_TYPES.has(result.type)) return false;
  const r = result as unknown as Record<string, unknown>;
  if (typeof r.value !== "string") return false;
  return (
    r.children === undefined &&
    r.position === undefined &&
    r.data === undefined &&
    r.lang === undefined &&
    r.meta === undefined
  );
}

/**
 * Walk an MDAST handle in Rust, dispatch matched nodes to JS visitor functions,
 * and collect mutations for the caller to apply. No arena buffers cross NAPI.
 *
 * Returns MdastVisitResult synchronously if all visitors are sync,
 * or Promise<MdastVisitResult> if any visitor is async.
 */
export function visitMdastHandle(
  handle: MdastHandle,
  plugin: MdastPluginInstance,
  subs: MdastSubscription[],
  source: string | (() => string),
  fileURL: URL | undefined,
  data: Data = {},
  sourceFormat: SourceFormat = "markdown",
  diagnostics: MdastDiagnostic[] = [],
): MdastVisitResult | Promise<MdastVisitResult> {
  const getSource = typeof source === "function" ? source : () => source;
  const resolver = new MdastLazyChildResolver(handle);
  const context = new MdastVisitorContextImpl(
    handle,
    getSource,
    fileURL,
    resolver,
    data,
    sourceFormat,
    diagnostics,
  );
  const rustSubs = getMdastRustSubs(plugin);
  const matchBuf: Uint8Array = walkMdastHandle(handle, rustSubs);
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = ru32(matchView, 0);

  let settling: Promise<void>[] | null = null;

  for (let i = 0; i < matchCount; i++) {
    const indexBase = 4 + i * 10;
    const nodeId = ru32(matchView, indexBase);
    const subIndex = matchBuf[indexBase + 4]!;
    const dataOffset = ru32(matchView, indexBase + 6);

    const sub = subs[subIndex]!;
    const node = readMdastMatchedNode(
      matchView,
      matchBuf,
      dataOffset,
      nodeId,
      sub.nodeType,
      resolver,
    );
    const result = sub.visitFn.call(plugin, node, context);

    if (result instanceof Promise) {
      // An async visitor's return is its last edit, so it queues when the visitor settles.
      (settling ??= []).push(result.then((settled) => context.applyReturn(settled, nodeId, node)));
    } else if (result !== undefined) {
      context.applyReturn(result, nodeId, node);
    }
  }

  if (settling)
    return Promise.all(settling).then(() => finalizeMdastVisit(handle, context, plugin));
  return finalizeMdastVisit(handle, context, plugin);
}

const MDAST_ROOT_SUBS: { nodeType: number; tagFilter: string[] }[] = [
  { nodeType: MDAST_ROOT, tagFilter: [] },
];

/** Hooks run in separate passes so each sees mutations from the preceding pass. */
export function visitMdastHook(
  handle: MdastHandle,
  plugin: MdastPluginInstance,
  hook: MdastHookFn,
  source: string | (() => string),
  fileURL: URL | undefined,
  data: Data = {},
  sourceFormat: SourceFormat = "markdown",
  diagnostics: MdastDiagnostic[] = [],
): MdastVisitResult | Promise<MdastVisitResult> {
  const getSource = typeof source === "function" ? source : () => source;
  const resolver = new MdastLazyChildResolver(handle);
  const context = new MdastVisitorContextImpl(
    handle,
    getSource,
    fileURL,
    resolver,
    data,
    sourceFormat,
    diagnostics,
  );
  const matchBuf: Uint8Array = walkMdastHandle(handle, MDAST_ROOT_SUBS);
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  if (ru32(matchView, 0) === 0) return finalizeMdastVisit(handle, context, plugin);

  const root = readMdastMatchedNode(
    matchView,
    matchBuf,
    ru32(matchView, 10),
    ru32(matchView, 4),
    MDAST_ROOT,
    resolver,
  ) as MdastRoot;

  const result = hook.call(plugin, root, context);
  if (result instanceof Promise) {
    return result.then(() => finalizeMdastVisit(handle, context, plugin));
  }
  return finalizeMdastVisit(handle, context, plugin);
}

function finalizeMdastVisit(
  handle: MdastHandle,
  context: MdastVisitorContextImpl,
  plugin: MdastPluginInstance,
): MdastVisitResult {
  context.removals.warn(plugin, (ids) => nodeLabels(handle, ids));
  const commandBuffer = takeCommands(context.getCommandBuffer());
  return {
    commandBuffer,
    diagnostics: context.getDiagnostics(),
    hasMutations: commandBuffer.length > 0,
  };
}
