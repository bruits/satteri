import { materializeNode } from "./mdast-materializer.js";
import { MdastReader } from "./mdast-reader.js";
import { CommandBuffer, classifyReturn, type StructuralOp } from "../command-buffer.js";
import { restorePhantomSpaces } from "../phantom.js";
import { ru16, ru32, rstr, readPosition } from "../wire-read.js";
import { decodeMdastTypeData } from "./generated/layout.js";
import {
  TYPE_NAMES,
  NAME_TO_TYPE,
  VISITOR_KEYS,
  MDAST_OPSTREAM_TYPES,
} from "./generated/node-types.js";
import {
  OpWriter,
  OF_VALUE,
  OF_URL,
  OF_TITLE,
  OF_ALT,
  OF_LANG,
  OF_META,
  OF_IDENTIFIER,
  OF_LABEL,
  OF_NAME,
  OF_REFERENCE_TYPE,
  OF_DEPTH,
  OF_CHECKED,
  OF_START,
  OF_ORDERED,
  OF_SPREAD,
  OF_EXPLICIT,
  PROP_STRING,
  emitMdxAttr,
} from "../op-stream.js";
import type { MdastNode, MdastNodeInternal, Toml, MathNode, InlineMath } from "../types.js";
import { walkMdastHandle, mdastTextContentHandle } from "#binding";
import { asArray, makeRequireNid, mergeAndReset } from "../visitor-shared.js";
import { LazyChildResolver } from "../lazy-child-resolver.js";
import { MdastChildStub } from "./child-stub.js";
import type { MdastHandle } from "../handles.js";
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
  Paragraph,
  Strong,
  Table,
  TableRow,
  TableCell,
  Text,
  ThematicBreak,
  Yaml,
} from "mdast";
import type { MdxJsxFlowElement, MdxJsxTextElement } from "../mdx-types.js";
import type { MdxFlowExpression, MdxTextExpression } from "../mdx-types.js";
import type { MdxjsEsm } from "../mdx-types.js";
import type { ContainerDirective, LeafDirective, TextDirective } from "../directive-types.js";

/** New content for a structural mutation: a declarative node, or a raw markdown
 *  / HTML escape hatch. Declarative nodes ride the op-stream when the type is
 *  supported, falling back to JSON otherwise — both produce identical arenas. */
export type MdastContent = MdastNode | { raw: string } | { rawHtml: string };

export interface MdastDiagnostic {
  message: string;
  nodeId?: number | undefined;
  position?: MdastNode["position"] | undefined;
  severity: "error" | "warning" | "info";
}

/** Maps MdastNode objects to their arena node IDs without Object.defineProperty overhead. */
const mdastNodeIdMap: WeakMap<object, number> = new WeakMap();

function nid(node: MdastNode): number | undefined {
  // Genuine stubs carry their id as a plain field; a spread copy is not
  // `instanceof` and has no `_nodeId`, so it correctly reads as new content.
  if (node instanceof MdastChildStub) return node._id;
  return mdastNodeIdMap.get(node as object) ?? (node as MdastNodeInternal)._nodeId;
}

const requireNid = makeRequireNid(nid);

export class MdastVisitorContext {
  readonly #commandBuffer: CommandBuffer = new CommandBuffer();
  readonly #diagnostics: MdastDiagnostic[] = [];
  readonly #handle: MdastHandle;
  readonly #getSource: () => string;
  /**
   * The URL of the document being processed (the compile `fileURL` option),
   * or `undefined` when none was given. Use `fileURLToPath(ctx.fileURL)` for a
   * decoded filesystem path.
   */
  readonly fileURL: URL | undefined;

  constructor(handle: MdastHandle, getSource: () => string, fileURL: URL | undefined) {
    this.#handle = handle;
    this.#getSource = getSource;
    this.fileURL = fileURL;
  }

  get source(): string {
    const value = this.#getSource();
    Object.defineProperty(this, "source", { value, writable: false, enumerable: true });
    return value;
  }

  removeNode(node: Readonly<MdastNode>): void {
    this.#commandBuffer.removeNode(requireNid(node as MdastNode, "removeNode"));
  }

  insertBefore(node: Readonly<MdastNode>, newNode: MdastContent | MdastContent[]): void {
    const id = requireNid(node as MdastNode, "insertBefore");
    for (const n of asArray(newNode)) emitMdastTree(this.#commandBuffer, "insertBefore", id, n);
  }

  insertAfter(node: Readonly<MdastNode>, newNode: MdastContent | MdastContent[]): void {
    const id = requireNid(node as MdastNode, "insertAfter");
    for (const n of asArray(newNode)) emitMdastTree(this.#commandBuffer, "insertAfter", id, n);
  }

  /**
   * Wrap `node` in `parentNode`, making it `parentNode`'s first child. Any
   * children `parentNode` declares are kept after it.
   */
  wrapNode(node: Readonly<MdastNode>, parentNode: MdastContent): void {
    const id = requireNid(node as MdastNode, "wrapNode");
    emitMdastTree(this.#commandBuffer, "wrapNode", id, parentNode);
  }

  prependChild(node: Readonly<MdastNode>, childNode: MdastContent | MdastContent[]): void {
    const id = requireNid(node as MdastNode, "prependChild");
    for (const n of asArray(childNode)) emitMdastTree(this.#commandBuffer, "prependChild", id, n);
  }

  appendChild(node: Readonly<MdastNode>, childNode: MdastContent | MdastContent[]): void {
    const id = requireNid(node as MdastNode, "appendChild");
    for (const n of asArray(childNode)) emitMdastTree(this.#commandBuffer, "appendChild", id, n);
  }

  /** Insert one node or an array at `index`; clamps (`0` or less prepends, past the end appends). */
  insertChildAt(
    node: Readonly<MdastNode>,
    index: number,
    childNode: MdastContent | MdastContent[],
  ): void {
    const children = "children" in node ? node.children : [];
    if (index <= 0 || children.length === 0) {
      this.prependChild(node, childNode);
    } else if (index >= children.length) {
      this.appendChild(node, childNode);
    } else {
      this.insertBefore(children[index]!, childNode);
    }
  }

  /** Remove the `index`-th child of `node`; a no-op when there is no such child. */
  removeChildAt(node: Readonly<MdastNode>, index: number): void {
    const child = "children" in node ? node.children[index] : undefined;
    if (child) this.removeNode(child);
  }

  replaceNode(node: Readonly<MdastNode>, newNode: MdastContent): void {
    const id = requireNid(node as MdastNode, "replaceNode");
    emitMdastTree(this.#commandBuffer, "replace", id, newNode, true);
  }

  setProperty<N extends MdastNode, K extends keyof N & string>(
    node: Readonly<N>,
    key: K,
    value: N[K],
  ): void {
    if (key === "children") {
      // children is structural: set-children keeps the node and swaps only its
      // child list (reused children keep their id).
      const id = requireNid(node as MdastNode, "setProperty");
      const ops = compileMdastChildrenToOpstream(value);
      if (ops) {
        this.#commandBuffer.setChildrenOpstream(id, ops);
        return;
      }
      const wrapper = refifyReusedNodes({ type: "root", children: value }, true);
      this.#commandBuffer.setChildren(id, JSON.stringify(wrapper));
      return;
    }
    if (key === "data") {
      // data is stored as JSON in the arena, serialize it for the command buffer
      this.#commandBuffer.setProperty(
        requireNid(node as MdastNode, "setProperty"),
        key,
        value != null ? JSON.stringify(value) : null,
      );
      return;
    }
    this.#commandBuffer.setProperty(requireNid(node as MdastNode, "setProperty"), key, value);
  }

  /** Collect the concatenated text of all descendant text nodes (like mdast-util-to-string). */
  textContent(
    node: Readonly<MdastNode>,
    options?: { includeImageAlt?: boolean; includeHtml?: boolean },
  ): string {
    return mdastTextContentHandle(
      this.#handle,
      requireNid(node as MdastNode, "textContent"),
      options,
    );
  }

  report({
    message,
    node,
    severity = "error",
  }: {
    message: string;
    node?: Readonly<MdastNode>;
    severity?: "error" | "warning" | "info";
  }): void {
    this.#diagnostics.push({
      message,
      nodeId: node ? nid(node) : undefined,
      position: node?.position,
      severity,
    });
  }

  /** Get the binary command buffer for all mutations recorded via context methods. */
  getCommandBuffer(): CommandBuffer {
    return this.#commandBuffer;
  }

  getDiagnostics(): MdastDiagnostic[] {
    return this.#diagnostics;
  }
}

type MdastVisitorResult =
  | MdastNode
  | { raw: string }
  | { rawHtml: string }
  | undefined
  | null
  | void;

type MdastVisitorFn<N extends MdastNode = MdastNode> = (
  node: Readonly<N>,
  context: MdastVisitorContext,
) => MdastVisitorResult | Promise<MdastVisitorResult>;

export interface MdastPluginInstance {
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
  mdxJsxFlowElement?: MdastVisitorFn<MdxJsxFlowElement>;
  mdxJsxTextElement?: MdastVisitorFn<MdxJsxTextElement>;
  mdxFlowExpression?: MdastVisitorFn<MdxFlowExpression>;
  mdxTextExpression?: MdastVisitorFn<MdxTextExpression>;
  mdxjsEsm?: MdastVisitorFn<MdxjsEsm>;
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
  visitFn: (node: MdastNode, context: MdastVisitorContext) => unknown;
}

export function resolveMdastSubscriptions(plugin: MdastPluginInstance): MdastSubscription[] {
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
  return subs;
}

class MdastLazyChildResolver extends LazyChildResolver<MdastReader, MdastNode> {
  protected override createReader(wire: Uint8Array): MdastReader {
    return new MdastReader(wire);
  }

  protected override materializeNode(reader: MdastReader, nodeId: number): MdastNode {
    return materializeNode(reader, nodeId);
  }
}

/** Build the child-stub list for a matched node from the wire's `[child_ids]
 *  [child_types]` blocks — no arena snapshot. The seal check still applies:
 *  post-pass ids are stale, and a stub built from them could later splice the
 *  wrong node as a ref. */
function readMdastChildStubs(
  view: DataView,
  buf: Uint8Array,
  idsPos: number,
  typesPos: number,
  count: number,
  resolver: MdastLazyChildResolver,
): MdastNode[] {
  resolver.assertUnsealed();
  const stubs: MdastNode[] = new Array(count);
  for (let i = 0; i < count; i++) {
    stubs[i] = new MdastChildStub(
      resolver,
      ru32(view, idsPos + i * 4),
      buf[typesPos + i]!,
    ) as unknown as MdastNode;
  }
  return stubs;
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
  resolver: MdastLazyChildResolver,
): void {
  Object.defineProperty(node, "children", {
    get(this: object): MdastNode[] {
      const val = readMdastChildStubs(view, buf, childIdsPos, childTypesPos, childCount, resolver);
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

/**
 * Read an MDAST node from the inline data in a match buffer entry.
 *
 * Inline format (from Rust serialize_mdast_node_inline):
 *   [node_data: u32+bytes][position: 6×u32 = 24B][child_count: u32][child_ids: N×u32]
 *   [child_types: N×u8][type-specific data]
 */
function readMdastMatchedNode(
  view: DataView,
  buf: Uint8Array,
  dataOffset: number,
  nodeId: number,
  nodeType: number,
  resolver: MdastLazyChildResolver,
): MdastNode {
  let pos = dataOffset;

  const dataJsonLen = ru32(view, pos);
  pos += 4;
  let initialData: Record<string, unknown> | null = null;
  if (dataJsonLen > 0) {
    const jsonStr = rstr(buf, pos, dataJsonLen);
    try {
      initialData = JSON.parse(jsonStr);
    } catch (err) {
      if (process.env.NODE_ENV !== "production") {
        console.warn(`readMdastMatchedNode: malformed node_data for nodeId=${nodeId}`, err);
      }
    }
    pos += dataJsonLen;
  }

  const position = readPosition(view, pos);
  pos += 24;

  const childCount = ru32(view, pos);
  pos += 4;
  // Ids/types decode lazily with `.children` — most matched nodes never read them.
  const childIdsPos = pos;
  pos += childCount * 4;
  const childTypesPos = pos;
  pos += childCount;

  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  const node: Record<string, unknown> = { type: typeName };
  if (position !== undefined) node.position = position;
  if (childCount > 0) {
    makeLazyChildren(node, view, buf, childIdsPos, childTypesPos, childCount, resolver);
  }

  // Fixed-field types decode from the generated layout table; the rest
  // (variable-length / cross-field) stay in the hand-written switch.
  if (!decodeMdastTypeData(view, buf, pos, nodeType, node)) {
    switch (nodeType) {
      case 5: {
        // list
        node.start = ru32(view, pos);
        node.ordered = buf[pos + 4]! !== 0;
        node.spread = buf[pos + 5]! !== 0;
        if (!node.ordered) node.start = null;
        break;
      }
      case 6: {
        // listItem
        const checked = buf[pos]!;
        node.checked = checked === 2 ? null : checked === 1;
        node.spread = buf[pos + 1]! !== 0;
        break;
      }
      case 21: {
        // table
        const count = ru16(view, pos);
        pos += 2;
        const alignNames: (string | null)[] = [null, "left", "right", "center"];
        node.align = Array.from({ length: count }, (_, i) => alignNames[buf[pos + i]!] ?? null);
        break;
      }
      case 30:
      case 31:
      case 32: {
        // containerDirective, leafDirective, textDirective
        const nameLen = ru16(view, pos);
        pos += 2;
        node.name = rstr(buf, pos, nameLen);
        pos += nameLen;
        const attrCount = ru16(view, pos);
        pos += 2;
        const attributes: Record<string, string> = {};
        for (let i = 0; i < attrCount; i++) {
          const keyLen = ru16(view, pos);
          pos += 2;
          const key = rstr(buf, pos, keyLen);
          pos += keyLen;
          const valLen = ru16(view, pos);
          pos += 2;
          const val = rstr(buf, pos, valLen);
          pos += valLen;
          attributes[key] = val;
        }
        node.attributes = attributes;
        break;
      }
      case 100:
      case 101: {
        // mdxJsxFlowElement, mdxJsxTextElement
        const nameLen = ru16(view, pos);
        pos += 2;
        node.name = nameLen > 0 ? rstr(buf, pos, nameLen) : null;
        pos += nameLen;
        const attrCount = ru16(view, pos);
        pos += 2;
        const attributes: { type: string; name?: string; value: unknown }[] = [];
        for (let i = 0; i < attrCount; i++) {
          const kind = buf[pos]!;
          pos += 1;
          const anLen = ru16(view, pos);
          pos += 2;
          const an = rstr(buf, pos, anLen);
          pos += anLen;
          const avLen = ru32(view, pos);
          pos += 4;
          const av = rstr(buf, pos, avLen);
          pos += avLen;
          switch (kind) {
            case 0:
              attributes.push({ type: "mdxJsxAttribute", name: an, value: null });
              break;
            case 1:
              attributes.push({ type: "mdxJsxAttribute", name: an, value: av });
              break;
            case 2:
              attributes.push({
                type: "mdxJsxAttribute",
                name: an,
                value: {
                  type: "mdxJsxAttributeValueExpression",
                  value: restorePhantomSpaces(av),
                },
              });
              break;
            case 3:
              attributes.push({
                type: "mdxJsxExpressionAttribute",
                value: restorePhantomSpaces(av),
              });
              break;
          }
        }
        node.attributes = attributes;
        break;
      }
      // root(0), paragraph(1), thematicBreak(3), blockquote(4), emphasis(11),
      // strong(12), break(14), tableRow(22), tableCell(23), delete(24): no extra data
    }
  }

  mdastNodeIdMap.set(node as object, nodeId);

  if (initialData) {
    (node as Record<string, unknown>).data = initialData;
  }

  return node as unknown as MdastNode;
}

/** The arena id of a node if it is an existing (materialized) node, else
 *  undefined for a freshly-built one. */
function reusedId(node: unknown): number | undefined {
  if (node === null || typeof node !== "object") return undefined;
  const id = nid(node as MdastNode);
  return typeof id === "number" ? id : undefined;
}

/**
 * Rewrite a returned replacement tree so every *reused* node (one that came
 * from the arena, at any depth) becomes a `{ _ref: id }` placeholder. The
 * rebuild splices those originals back in place, preserving their ids — so a
 * patch a nested visitor queued on a passed-through child still lands, in the
 * same pass. Freshly-built nodes serialize as before. The root is never reffed:
 * it is the new shape replacing the visited node.
 */
function refifyReusedNodes(node: unknown, isRoot: boolean): MdastNode {
  if (node === null || typeof node !== "object") return node as MdastNode;
  if (!isRoot) {
    const id = reusedId(node);
    if (id !== undefined) return { _ref: id } as unknown as MdastNode;
  }
  const children = (node as { children?: unknown }).children;
  if (Array.isArray(children)) {
    return {
      ...(node as object),
      children: children.map((c) => refifyReusedNodes(c, false)),
    } as unknown as MdastNode;
  }
  return node as MdastNode;
}

// Reused across every replacement in a pass: compile is synchronous and its
// result is copied into the command buffer before the next call, so a single
// writer is safe and avoids a 512-byte allocation per built node.
const mdastWriter = new OpWriter();

/**
 * Compile a declarative MDAST replacement tree to the op-stream — the structural
 * backend the rebuild already uses, but skipping JSON + the JsNode deserialize.
 * Reused nodes (those still carrying an arena id) become `ref`s, exactly like
 * `refifyReusedNodes`. Returns null when the replay can't reproduce the tree
 * identically (unsupported node type, out-of-range numeric field, or a
 * `_keepChildren` marker outside a replace), so the caller falls back to JSON.
 */
function compileMdastToOpstream(root: unknown, forReplace = false): Uint8Array | null {
  mdastWriter.reset();
  if (!emitMdastOp(mdastWriter, root, true, forReplace)) return null;
  return mdastWriter.take();
}

/** Compile a set-children payload: a root-wrapped child list, the shape
 *  `Patch::SetChildren` splices in. Reused children become refs, like the
 *  JSON wrapper's `refifyReusedNodes`. */
function compileMdastChildrenToOpstream(children: unknown): Uint8Array | null {
  if (!Array.isArray(children)) return null;
  mdastWriter.reset();
  mdastWriter.open(NAME_TO_TYPE.root!);
  for (const c of children) {
    if (!emitMdastOp(mdastWriter, c, false, false)) return null;
  }
  mdastWriter.close();
  return mdastWriter.take();
}

function emitMdastOp(w: OpWriter, node: unknown, isRoot: boolean, forReplace: boolean): boolean {
  if (node === null || typeof node !== "object") return false;
  if (!isRoot) {
    const id = reusedId(node);
    if (id !== undefined) {
      w.ref(id);
      return true;
    }
  }
  const n = node as Record<string, unknown>;
  const type = MDAST_OPSTREAM_TYPES[n.type as string];
  if (type === undefined) return false;
  w.open(type);
  if (typeof n.value === "string") w.str(OF_VALUE, n.value);
  if (typeof n.url === "string") w.str(OF_URL, n.url);
  if (typeof n.title === "string") w.str(OF_TITLE, n.title);
  if (typeof n.alt === "string") w.str(OF_ALT, n.alt);
  if (typeof n.lang === "string") w.str(OF_LANG, n.lang);
  if (typeof n.meta === "string") w.str(OF_META, n.meta);
  if (typeof n.identifier === "string") w.str(OF_IDENTIFIER, n.identifier);
  if (typeof n.label === "string") w.str(OF_LABEL, n.label);
  if (typeof n.referenceType === "string") w.str(OF_REFERENCE_TYPE, n.referenceType);
  // Out-of-range numbers fall back to JSON, where serde's Option<u8>/Option<u32>
  // rejects the node with a visible error instead of silently masking the bits.
  if (typeof n.depth === "number") {
    if (!Number.isInteger(n.depth) || n.depth < 0 || n.depth > 255) return false;
    w.u8(OF_DEPTH, n.depth);
  }
  if (typeof n.checked === "boolean") w.u8(OF_CHECKED, n.checked ? 1 : 0);
  if (typeof n.start === "number") {
    if (!Number.isInteger(n.start) || n.start < 0 || n.start > 4294967295) return false;
    w.u32(OF_START, n.start);
  }
  if (typeof n.ordered === "boolean") w.bool(OF_ORDERED, n.ordered);
  if (typeof n.spread === "boolean") w.bool(OF_SPREAD, n.spread);
  if (typeof n.name === "string") w.str(OF_NAME, n.name);
  const attrs = n.attributes;
  if (Array.isArray(attrs)) {
    for (const a of attrs) emitMdxAttr(w, a as Record<string, unknown>);
  } else if (attrs !== null && typeof attrs === "object") {
    // Directive attributes: a string→string map. Non-string values are dropped,
    // matching the JSON path's `encode_js_directive_attrs`.
    for (const key in attrs as Record<string, unknown>) {
      const v = (attrs as Record<string, unknown>)[key];
      if (typeof v === "string") w.prop(key, PROP_STRING, v);
    }
  }
  if (Array.isArray(n.align)) w.align(n.align.map(alignCode));
  if ((n.data as Record<string, unknown> | null | undefined)?._mdxExplicitJsx === true) {
    w.bool(OF_EXPLICIT, true);
  }
  if (n.data != null) w.data(n.data);
  if (isRoot && n._keepChildren === true) {
    // Replace splices the target's original children, discarding any the
    // replacement declares — same as the JSON path's keep_children. Other
    // commands ignore the marker, so fall back to JSON for exact parity.
    if (!forReplace) return false;
    w.keepChildren();
  } else {
    const children = n.children;
    if (Array.isArray(children)) {
      for (const c of children) if (!emitMdastOp(w, c, false, forReplace)) return false;
    }
  }
  w.close();
  return true;
}

/** Map a table `align` entry to its arena code (none=0). */
function alignCode(a: unknown): number {
  return a === "left" ? 1 : a === "right" ? 2 : a === "center" ? 3 : 0;
}

/**
 * Resolve declarative content to either an op-stream (fast path) or a
 * JSON-ready node (raw escape hatches + unsupported types). The op-stream and
 * JSON paths produce identical arenas; this just picks the cheaper encoding.
 */
function compileOrJson(content: MdastContent, forReplace = false): Uint8Array | MdastNode {
  return compileMdastToOpstream(content, forReplace) ?? refifyReusedNodes(content, true);
}

/** Encode `content` (op-stream when compilable, JSON otherwise) and write it
 *  as the `op` structural command — the one place that picks the encoding. The
 *  switch keeps the buffer calls monomorphic (computed method names allocate
 *  and defeat inline caches on this warm path). */
function emitMdastTree(
  buffer: CommandBuffer,
  op: StructuralOp,
  id: number,
  content: MdastContent,
  forReplace = false,
): void {
  const c = compileOrJson(content, forReplace);
  if (c instanceof Uint8Array) {
    switch (op) {
      case "replace":
        return buffer.replaceOpstream(id, c);
      case "insertBefore":
        return buffer.insertBeforeOpstream(id, c);
      case "insertAfter":
        return buffer.insertAfterOpstream(id, c);
      case "prependChild":
        return buffer.prependChildOpstream(id, c);
      case "appendChild":
        return buffer.appendChildOpstream(id, c);
      case "wrapNode":
        return buffer.wrapNodeOpstream(id, c);
    }
  }
  switch (op) {
    case "replace":
      return buffer.replace(id, c);
    case "insertBefore":
      return buffer.insertBefore(id, c);
    case "insertAfter":
      return buffer.insertAfter(id, c);
    case "prependChild":
      return buffer.prependChild(id, c);
    case "appendChild":
      return buffer.appendChild(id, c);
    case "wrapNode":
      return buffer.wrapNode(id, c);
  }
}

/** A result that is the same object as the input node is a no-op, so context
 *  mutations (e.g. setProperty) are not clobbered. */
function applyMdastVisitResult(
  result: MdastVisitorResult,
  nodeId: number,
  returnBuffer: CommandBuffer,
  originalNode?: MdastNode,
): void {
  if (result === undefined || result === null) return;
  if (result === originalNode) return;
  const cls = classifyReturn(result);
  switch (cls) {
    case "raw_markdown":
      returnBuffer.replace(nodeId, result as unknown as { raw: string });
      break;
    case "raw_html":
      returnBuffer.replace(nodeId, result as unknown as { rawHtml: string });
      break;
    case "structured_node":
      emitMdastTree(returnBuffer, "replace", nodeId, result as MdastContent, true);
      break;
  }
}

/**
 * Walk an MDAST handle in Rust, dispatch matched nodes to JS visitor functions,
 * and apply mutations back to the handle. No arena buffers cross NAPI.
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
): MdastVisitResult | Promise<MdastVisitResult> {
  const getSource = typeof source === "function" ? source : () => source;
  const context = new MdastVisitorContext(handle, getSource, fileURL);
  const returnBuffer = new CommandBuffer();
  const resolver = new MdastLazyChildResolver(handle);
  const rustSubs = subs.map((s) => ({ nodeType: s.nodeType, tagFilter: [] as string[] }));
  const matchBuf: Uint8Array = walkMdastHandle(handle, rustSubs);
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = ru32(matchView, 0);

  let deferred:
    | { nodeId: number; promise: Promise<MdastVisitorResult>; originalNode: MdastNode }[]
    | null = null;

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
      deferred ??= [];
      deferred.push({ nodeId, promise: result, originalNode: node });
    } else {
      applyMdastVisitResult(result as MdastVisitorResult, nodeId, returnBuffer, node);
    }
  }

  if (deferred) {
    return Promise.all(
      deferred.map((d) =>
        d.promise.then((r) => ({ nodeId: d.nodeId, result: r, originalNode: d.originalNode })),
      ),
    ).then((results) => {
      for (const { nodeId, result, originalNode } of results) {
        applyMdastVisitResult(result, nodeId, returnBuffer, originalNode);
      }
      // End of the pass — the caller applies the returned command buffer next,
      // renumbering the arena, so later snapshots would resolve match-time
      // child ids against wrong nodes. This is the last point we control.
      resolver.seal();
      return finalizeMdastVisit(handle, context, returnBuffer);
    });
  }

  resolver.seal();
  return finalizeMdastVisit(handle, context, returnBuffer);
}

function finalizeMdastVisit(
  handle: MdastHandle,
  context: MdastVisitorContext,
  returnBuffer: CommandBuffer,
): MdastVisitResult {
  const { merged, hasMutations } = mergeAndReset(returnBuffer, context);
  return { commandBuffer: merged, diagnostics: context.getDiagnostics(), hasMutations };
}
