import { materializeNode, TYPE_NAMES } from "./mdast-materializer.js";
import { MdastReader } from "./mdast-reader.js";
import { CommandBuffer, classifyReturn } from "../command-buffer.js";
import type { MdastNode, MdastNodeInternal, Toml, MathNode, InlineMath } from "../types.js";
import {
  walkMdastHandle,
  serializeHandle,
  getNodeData as napiGetNodeData,
  mdastTextContentHandle,
} from "#binding";
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

const MutationType = {
  Replace: "replace",
  Remove: "remove",
  InsertBefore: "insertBefore",
  InsertAfter: "insertAfter",
  Wrap: "wrap",
  PrependChild: "prependChild",
  AppendChild: "appendChild",
  SetProperty: "setProperty",
} as const;

type MutationTypeValue = (typeof MutationType)[keyof typeof MutationType];

interface Mutation {
  type: MutationTypeValue;
  nodeId: number;
  newNode?: MdastNode;
  key?: string;
  value?: unknown;
}

export interface MdastDiagnostic {
  message: string;
  nodeId?: number | undefined;
  position?: MdastNode["position"] | undefined;
  severity: "error" | "warning" | "info";
}

const VISITOR_KEYS = new Set([
  "paragraph",
  "heading",
  "thematicBreak",
  "blockquote",
  "list",
  "listItem",
  "html",
  "code",
  "definition",
  "text",
  "emphasis",
  "strong",
  "inlineCode",
  "break",
  "link",
  "image",
  "linkReference",
  "imageReference",
  "footnoteDefinition",
  "footnoteReference",
  "table",
  "tableRow",
  "tableCell",
  "delete",
  "yaml",
  "toml",
  "math",
  "inlineMath",
  "containerDirective",
  "leafDirective",
  "textDirective",
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
]);

/** Maps MdastNode objects to their arena node IDs without Object.defineProperty overhead. */
const mdastNodeIdMap: WeakMap<object, number> = new WeakMap();

function nid(node: MdastNode): number {
  return mdastNodeIdMap.get(node as object) ?? (node as MdastNodeInternal)._nodeId;
}

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
    this.#commandBuffer.removeNode(nid(node as MdastNode));
  }

  insertBefore(node: Readonly<MdastNode>, newNode: MdastNode): void {
    this.#commandBuffer.insertBefore(nid(node as MdastNode), newNode);
  }

  insertAfter(node: Readonly<MdastNode>, newNode: MdastNode): void {
    this.#commandBuffer.insertAfter(nid(node as MdastNode), newNode);
  }

  wrapNode(node: Readonly<MdastNode>, parentNode: MdastNode): void {
    this.#commandBuffer.wrapNode(nid(node as MdastNode), parentNode);
  }

  prependChild(node: Readonly<MdastNode>, childNode: MdastNode): void {
    this.#commandBuffer.prependChild(nid(node as MdastNode), childNode);
  }

  appendChild(node: Readonly<MdastNode>, childNode: MdastNode): void {
    this.#commandBuffer.appendChild(nid(node as MdastNode), childNode);
  }

  replaceNode(node: Readonly<MdastNode>, newNode: MdastNode): void {
    this.#commandBuffer.replace(nid(node as MdastNode), newNode);
  }

  setProperty<N extends MdastNode, K extends keyof N & string>(
    node: Readonly<N>,
    key: K,
    value: N[K],
  ): void {
    if (key === "data") {
      // data is stored as JSON in the arena, serialize it for the command buffer
      this.#commandBuffer.setProperty(
        nid(node as MdastNode),
        key,
        value != null ? JSON.stringify(value) : null,
      );
      return;
    }
    this.#commandBuffer.setProperty(nid(node as MdastNode), key, value);
  }

  /** Collect the concatenated text of all descendant text nodes (like mdast-util-to-string). */
  textContent(
    node: Readonly<MdastNode>,
    options?: { includeImageAlt?: boolean; includeHtml?: boolean },
  ): string {
    return mdastTextContentHandle(this.#handle, nid(node as MdastNode), options);
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

/** Merge return-value + context command buffers and release internals. */
function mergeAndReset(
  returnBuffer: CommandBuffer,
  ctx: MdastVisitorContext,
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

// Handle-based MDAST visitor (arena stays in Rust)

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type MdastHandle = any;

const textDecoder = new TextDecoder("utf-8");

/** Build name→nodeType map from TYPE_NAMES (reverse of TYPE_NAMES). */
const NAME_TO_TYPE: Record<string, number> = {};
for (const [num, name] of Object.entries(TYPE_NAMES)) {
  NAME_TO_TYPE[name] = Number(num);
}

interface MdastSubscription {
  nodeType: number;
  visitFn: (node: MdastNode, context: MdastVisitorContext) => unknown;
}

/** Resolve subscriptions from a plugin instance. */
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

/** Read a u16 from buf at offset (LE). */
function ru16(view: DataView, off: number): number {
  return view.getUint16(off, true);
}
/** Read a u32 from buf at offset (LE). */
function ru32(view: DataView, off: number): number {
  return view.getUint32(off, true);
}
/** Read a utf8 string from buf. */
function rstr(buf: Uint8Array, off: number, len: number): string {
  return len === 0 ? "" : textDecoder.decode(buf.subarray(off, off + len));
}

/**
 * Lazy child materializer for the MDAST handle walk path.
 * Serializes the handle once on first child access, then materializes
 * children via MdastReader + materializeNode.
 */
class MdastLazyChildResolver {
  #handle: MdastHandle;
  #reader: MdastReader | null = null;

  constructor(handle: MdastHandle) {
    this.#handle = handle;
  }

  #ensure(): MdastReader {
    if (!this.#reader) {
      this.#reader = new MdastReader(serializeHandle(this.#handle));
    }
    return this.#reader;
  }

  materializeChildren(childIds: number[]): MdastNode[] {
    const reader = this.#ensure();
    const handle = this.#handle;
    return childIds.map((id) => {
      const node = materializeNode(reader, id);
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

/**
 * Read an MDAST node from the inline data in a match buffer entry.
 *
 * Inline format (from Rust serialize_mdast_node_inline):
 *   [node_data: u32+bytes][position: 6×u32 = 24B][child_count: u16][child_ids: N×u32][type-specific data]
 */
const encoder = new TextEncoder();

function readMdastMatchedNode(
  view: DataView,
  buf: Uint8Array,
  dataOffset: number,
  nodeId: number,
  nodeType: number,
  resolver: MdastLazyChildResolver,
): MdastNode {
  let pos = dataOffset;

  // Node data (JSON bytes), always first
  const dataJsonLen = ru32(view, pos);
  pos += 4;
  let initialData: Record<string, unknown> | null = null;
  if (dataJsonLen > 0) {
    const jsonStr = rstr(buf, pos, dataJsonLen);
    try {
      initialData = JSON.parse(jsonStr);
    } catch {
      /* ignore */
    }
    pos += dataJsonLen;
  }

  // Position
  const position = {
    start: { offset: ru32(view, pos), line: ru32(view, pos + 8), column: ru32(view, pos + 12) },
    end: { offset: ru32(view, pos + 4), line: ru32(view, pos + 16), column: ru32(view, pos + 20) },
  };
  pos += 24;

  // Children, read IDs, materialize lazily via resolver
  const childCount = ru16(view, pos);
  pos += 2;
  const childIds: number[] = [];
  for (let i = 0; i < childCount; i++) {
    childIds.push(ru32(view, pos));
    pos += 4;
  }

  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  // Build node with type-specific fields
  const node: Record<string, unknown> = { type: typeName, position };
  if (childCount > 0) {
    Object.defineProperty(node, "children", {
      get() {
        const val = resolver.materializeChildren(childIds);
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
  }

  switch (nodeType) {
    case 2: {
      // heading
      node.depth = buf[pos]!;
      break;
    }
    case 10:
    case 13:
    case 7:
    case 25:
    case 26: {
      // text, inlineCode, html, yaml, toml
      const vlen = ru32(view, pos);
      node.value = rstr(buf, pos + 4, vlen);
      break;
    }
    case 8: {
      // code
      const langLen = ru16(view, pos);
      pos += 2;
      node.lang = langLen > 0 ? rstr(buf, pos, langLen) : null;
      pos += langLen;
      const metaLen = ru16(view, pos);
      pos += 2;
      node.meta = metaLen > 0 ? rstr(buf, pos, metaLen) : null;
      pos += metaLen;
      const valLen = ru32(view, pos);
      pos += 4;
      node.value = rstr(buf, pos, valLen);
      break;
    }
    case 27:
    case 28: {
      // math, inlineMath
      const metaLen = ru16(view, pos);
      pos += 2;
      node.meta = metaLen > 0 ? rstr(buf, pos, metaLen) : null;
      pos += metaLen;
      const valLen = ru32(view, pos);
      pos += 4;
      node.value = rstr(buf, pos, valLen);
      break;
    }
    case 15: {
      // link
      const urlLen = ru16(view, pos);
      pos += 2;
      node.url = rstr(buf, pos, urlLen);
      pos += urlLen;
      const titleLen = ru16(view, pos);
      pos += 2;
      node.title = titleLen > 0 ? rstr(buf, pos, titleLen) : null;
      break;
    }
    case 16: {
      // image
      const urlLen = ru16(view, pos);
      pos += 2;
      node.url = rstr(buf, pos, urlLen);
      pos += urlLen;
      const altLen = ru16(view, pos);
      pos += 2;
      node.alt = rstr(buf, pos, altLen);
      pos += altLen;
      const titleLen = ru16(view, pos);
      pos += 2;
      node.title = titleLen > 0 ? rstr(buf, pos, titleLen) : null;
      break;
    }
    case 9: {
      // definition
      const urlLen = ru16(view, pos);
      pos += 2;
      node.url = rstr(buf, pos, urlLen);
      pos += urlLen;
      const titleLen = ru16(view, pos);
      pos += 2;
      node.title = titleLen > 0 ? rstr(buf, pos, titleLen) : null;
      pos += titleLen;
      const idLen = ru16(view, pos);
      pos += 2;
      node.identifier = rstr(buf, pos, idLen);
      pos += idLen;
      const labelLen = ru16(view, pos);
      pos += 2;
      node.label = rstr(buf, pos, labelLen);
      break;
    }
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
    case 17:
    case 18:
    case 20: {
      // linkReference, imageReference, footnoteReference
      const idLen = ru16(view, pos);
      pos += 2;
      node.identifier = rstr(buf, pos, idLen);
      pos += idLen;
      const labelLen = ru16(view, pos);
      pos += 2;
      node.label = rstr(buf, pos, labelLen);
      pos += labelLen;
      const kind = buf[pos]!;
      // Only link/image references carry `referenceType`; the mdast spec
      // defines it for those two, not for `footnoteReference`.
      if (nodeType !== 20) {
        node.referenceType = ["shortcut", "collapsed", "full"][kind] ?? "shortcut";
      }
      break;
    }
    case 19: {
      // footnoteDefinition
      const idLen = ru16(view, pos);
      pos += 2;
      node.identifier = rstr(buf, pos, idLen);
      pos += idLen;
      const labelLen = ru16(view, pos);
      pos += 2;
      node.label = rstr(buf, pos, labelLen);
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
              value: { type: "mdxJsxAttributeValueExpression", value: av },
            });
            break;
          case 3:
            attributes.push({ type: "mdxJsxExpressionAttribute", value: av });
            break;
        }
      }
      node.attributes = attributes;
      break;
    }
    case 102:
    case 103:
    case 104: {
      // mdxFlowExpression, mdxTextExpression, mdxjsEsm
      const vlen = ru32(view, pos);
      node.value = rstr(buf, pos + 4, vlen);
      break;
    }
    // root(0), paragraph(1), thematicBreak(3), blockquote(4), emphasis(11),
    // strong(12), break(14), tableRow(22), tableCell(23), delete(24): no extra data
  }

  mdastNodeIdMap.set(node as object, nodeId);

  if (initialData) {
    (node as Record<string, unknown>).data = initialData;
  }

  return node as unknown as MdastNode;
}

/** Apply a sync visitor result to the return buffer.
 *  If the result is the same object as the input node, treat it as a no-op
 *  so that context mutations (e.g. setProperty) are not clobbered. */
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
function refifyReusedNodes(node: unknown, isRoot: boolean): unknown {
  if (node === null || typeof node !== "object") return node;
  if (!isRoot) {
    const id = reusedId(node);
    if (id !== undefined) return { _ref: id };
  }
  const children = (node as { children?: unknown }).children;
  if (Array.isArray(children)) {
    return { ...(node as object), children: children.map((c) => refifyReusedNodes(c, false)) };
  }
  return node;
}

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
      returnBuffer.replace(nodeId, refifyReusedNodes(result, true) as MdastNode);
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
    const indexBase = 4 + i * 12;
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
      return finalizeMdastVisit(handle, context, returnBuffer);
    });
  }

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
