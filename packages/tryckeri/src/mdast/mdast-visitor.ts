import { materializeNode, TYPE_NAMES } from "./mdast-materializer.js";
import { CommandBuffer, classifyReturn } from "../command-buffer.js";
import type { MdastNode, MdastNodeInternal, Toml, MathNode, InlineMath } from "../types.js";
import {
  walkMdastHandle,
  applyCommandsToMdastHandle,
  getHandleSource,
  setNodeData,
} from "../../index.js";
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
  Root,
  Strong,
  Table,
  TableRow,
  TableCell,
  Text,
  ThematicBreak,
  Yaml,
} from "mdast";
import type { MdxJsxFlowElement, MdxJsxTextElement } from "mdast-util-mdx-jsx";
import type { MdxFlowExpression, MdxTextExpression } from "mdast-util-mdx-expression";
import type { MdxjsEsm } from "mdast-util-mdxjs-esm";
import type { MdastReader } from "./mdast-reader.js";
import { DataMap } from "../data-map.js";

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
  "root",
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
  "mdxJsxFlowElement",
  "mdxJsxTextElement",
  "mdxFlowExpression",
  "mdxTextExpression",
  "mdxjsEsm",
]);

function nid(node: MdastNode): number {
  return (node as MdastNodeInternal)._nodeId;
}

export class MdastVisitorContext {
  readonly #commandBuffer: CommandBuffer = new CommandBuffer();
  readonly #diagnostics: MdastDiagnostic[] = [];
  readonly #reader: MdastReader;
  readonly #dataMap: DataMap;
  readonly #rootId: number = 0;

  constructor(reader: MdastReader, dataMap: DataMap) {
    this.#reader = reader;
    this.#dataMap = dataMap;
  }

  removeNode(node: MdastNode): void {
    this.#commandBuffer.removeNode(nid(node));
  }

  insertBefore(node: MdastNode, newNode: MdastNode): void {
    this.#commandBuffer.insertBefore(nid(node), newNode);
  }

  insertAfter(node: MdastNode, newNode: MdastNode): void {
    this.#commandBuffer.insertAfter(nid(node), newNode);
  }

  wrapNode(node: MdastNode, parentNode: MdastNode): void {
    this.#commandBuffer.wrapNode(nid(node), parentNode);
  }

  prependChild(node: MdastNode, childNode: MdastNode): void {
    this.#commandBuffer.prependChild(nid(node), childNode);
  }

  appendChild(node: MdastNode, childNode: MdastNode): void {
    this.#commandBuffer.appendChild(nid(node), childNode);
  }

  replaceNode(node: MdastNode, newNode: MdastNode): void {
    this.#commandBuffer.replace(nid(node), newNode);
  }

  setProperty(node: MdastNode, key: string, value: unknown): void {
    this.#commandBuffer.setProperty(nid(node), key, value);
  }

  report({
    message,
    node,
    severity = "error",
  }: {
    message: string;
    node?: MdastNode;
    severity?: "error" | "warning" | "info";
  }): void {
    this.#diagnostics.push({
      message,
      nodeId: node ? nid(node) : undefined,
      position: node?.position,
      severity,
    });
  }

  get root(): MdastNode {
    return materializeNode(this.#reader, this.#rootId, this.#dataMap);
  }

  get source(): string {
    return this.#reader.getSource();
  }

  /** Get the binary command buffer for all mutations recorded via context methods. */
  getCommandBuffer(): CommandBuffer {
    return this.#commandBuffer;
  }

  getDiagnostics(): MdastDiagnostic[] {
    return this.#diagnostics;
  }
}

type MdastVisitorFn<N extends MdastNode = MdastNode> = (
  node: N,
  context: MdastVisitorContext,
) => MdastNode | { raw: string } | { rawHtml: string } | undefined | null | void;

export interface MdastPluginInstance {
  before?(context: MdastVisitorContext): void;
  after?(context: MdastVisitorContext): void;
  transformRoot?(root: Root, context: MdastVisitorContext): MdastNode | undefined | null;
  root?: MdastVisitorFn<Root>;
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

/**
 * Walk the MDAST and dispatch to plugin visitor functions.
 *
 * Mutations are collected into a binary command buffer. Return values from
 * visitor functions are classified (raw/rawHtml/structured) and encoded
 * as REPLACE commands in the buffer.
 */
export function visitMdast(
  reader: MdastReader,
  plugin: MdastPluginInstance,
  dataMap: DataMap,
): MdastVisitResult {
  const context = new MdastVisitorContext(reader, dataMap);

  plugin.before?.(context);

  // Separate CommandBuffer for return-value mutations (replace commands from
  // visitor return values). These are merged with the context's buffer at the end.
  const returnBuffer = new CommandBuffer();

  if (typeof plugin.transformRoot === "function") {
    // Full materialization path
    const root = materializeNode(reader, 0, dataMap) as Root;
    const result = plugin.transformRoot(root, context);
    if (result !== undefined && result !== null) {
      const cls = classifyReturn(result);
      switch (cls) {
        case "raw_markdown":
          returnBuffer.replace(0, result as unknown as { raw: string });
          break;
        case "raw_html":
          returnBuffer.replace(0, result as unknown as { rawHtml: string });
          break;
        case "structured_node":
          returnBuffer.replace(0, result);
          break;
        // no_change: do nothing
      }
    }
  } else {
    // Fast path: walk raw bytes, only materialize subscribed node types

    // Build reverse map: numeric type → visitor function
    const TYPE_TO_VISITOR = new Map<
      number,
      (node: MdastNode, context: MdastVisitorContext) => unknown
    >();
    for (const [name, fn] of Object.entries(plugin)) {
      if (VISITOR_KEYS.has(name) && typeof fn === "function") {
        for (const [num, typeName] of Object.entries(TYPE_NAMES)) {
          if (typeName === name) {
            TYPE_TO_VISITOR.set(
              Number(num),
              fn as (node: MdastNode, context: MdastVisitorContext) => unknown,
            );
            break;
          }
        }
      }
    }

    // Walk raw buffer — only type-check each node, materialize only on subscription match
    const stack: number[] = [0];
    while (stack.length > 0) {
      const nodeId = stack.pop()!;
      const nodeType = reader.getNodeType(nodeId);

      const visitor = TYPE_TO_VISITOR.get(nodeType);
      if (visitor) {
        const node = materializeNode(reader, nodeId, dataMap);
        const result = visitor.call(plugin, node, context);
        if (result !== undefined && result !== null) {
          const cls = classifyReturn(result);
          switch (cls) {
            case "raw_markdown":
              returnBuffer.replace(nodeId, result as unknown as { raw: string });
              break;
            case "raw_html":
              returnBuffer.replace(nodeId, result as unknown as { rawHtml: string });
              break;
            case "structured_node":
              returnBuffer.replace(nodeId, result as MdastNode);
              break;
            // no_change: do nothing
          }
        }
      }

      reader.pushChildIds(nodeId, stack);
    }
  }

  plugin.after?.(context);

  const { merged, hasMutations } = mergeAndReset(returnBuffer, context);
  return {
    commandBuffer: merged,
    diagnostics: context.getDiagnostics(),
    hasMutations,
  };
}

// ---------------------------------------------------------------------------
// Handle-based MDAST visitor (arena stays in Rust)
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type MdastHandle = any;

const textDecoder = new TextDecoder("utf-8");

function isChildRefArray(children: unknown): boolean {
  if (!Array.isArray(children) || children.length === 0) return false;
  return children.every((c: Record<string, unknown>) => c?.type === "__child_ref__");
}

/** Build name→nodeType map from TYPE_NAMES (reverse of TYPE_NAMES). */
const NAME_TO_TYPE: Record<string, number> = {};
for (const [num, name] of Object.entries(TYPE_NAMES)) {
  NAME_TO_TYPE[name] = Number(num);
}

interface MdastSubscription {
  nodeType: number;
  visitFn: (node: MdastNode, context: MdastVisitorContext) => unknown;
}

/**
 * Resolve subscriptions from a plugin. Returns null if the plugin uses
 * transformRoot (needs buffer fallback).
 */
export function resolveMdastSubscriptions(plugin: MdastPluginInstance): MdastSubscription[] | null {
  if (plugin.transformRoot) return null;

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
  return subs.length > 0 ? subs : null;
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
 * Read an MDAST node from the inline data in a match buffer entry.
 *
 * Inline format (from Rust serialize_mdast_node_inline):
 *   [position: 6×u32 = 24B][child_count: u16][child_ids: N×u32][type-specific data]
 */
const encoder = new TextEncoder();

function readMdastMatchedNode(
  view: DataView,
  buf: Uint8Array,
  dataOffset: number,
  nodeId: number,
  nodeType: number,
  dirtyData: Map<number, Record<string, unknown>>,
): MdastNode {
  let pos = dataOffset;

  // Node data (JSON bytes) — always first
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

  // Children (opaque refs)
  const childCount = ru16(view, pos);
  pos += 2;
  const children: { _nodeId: number; type: string }[] = [];
  for (let i = 0; i < childCount; i++) {
    children.push({ _nodeId: ru32(view, pos), type: "__child_ref__" });
    pos += 4;
  }

  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  // Build node with type-specific fields
  const node: Record<string, unknown> = { type: typeName, position };
  if (childCount > 0) node.children = children;

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
    case 26:
    case 28: {
      // text, inlineCode, html, yaml, toml, inlineMath
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
    case 27: {
      // math
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
      node.referenceType = ["shortcut", "collapsed", "full"][kind] ?? "shortcut";
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
        const avLen = ru16(view, pos);
        pos += 2;
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

  Object.defineProperty(node, "_nodeId", { value: nodeId, enumerable: false });

  // Read inline node_data JSON from the end of the data section.
  // The Rust serializer always appends [json_len: u32][json_bytes...] at the end.
  // We need to find it by reading from dataEnd backwards.
  // Actually, we read it at `dataOffset + dataLen - (4 + jsonDataLen)` but we don't
  // know jsonDataLen yet. Instead, the data section ends with [len: u32][bytes...],
  // so we scan from the current end of type-specific data.
  // For simplicity, we'll handle this in the caller which knows the full data_len.

  // Set up data getter/setter that tracks dirty entries
  let currentData: Record<string, unknown> | null = initialData;
  Object.defineProperty(node, "data", {
    get() {
      return currentData;
    },
    set(value: Record<string, unknown> | null) {
      currentData = value;
      dirtyData.set(nodeId, value!);
    },
    configurable: true,
    enumerable: true,
  });

  return node as unknown as MdastNode;
}

/**
 * Walk an MDAST handle in Rust, dispatch matched nodes to JS visitor functions,
 * and apply mutations back to the handle. No arena buffers cross NAPI.
 */
export function visitMdastHandle(
  handle: MdastHandle,
  plugin: MdastPluginInstance,
  subs: MdastSubscription[],
  dataMap?: DataMap,
): MdastVisitResult {
  const dm = dataMap ?? new DataMap();
  const context = new MdastVisitorContext(null as unknown as MdastReader, dm);
  const returnBuffer = new CommandBuffer();
  const dirtyData = new Map<number, Record<string, unknown>>();

  plugin.before?.(context);

  // Build Rust subscriptions (no tag filter for MDAST — all matched by type)
  const rustSubs = subs.map((s) => ({ nodeType: s.nodeType, tagFilter: [] as string[] }));
  const matchBuf: Uint8Array = walkMdastHandle(handle, rustSubs);
  const matchView = new DataView(matchBuf.buffer, matchBuf.byteOffset, matchBuf.byteLength);
  const matchCount = ru32(matchView, 0);

  for (let i = 0; i < matchCount; i++) {
    const indexBase = 4 + i * 12;
    const nodeId = ru32(matchView, indexBase);
    const subIndex = matchBuf[indexBase + 4]!;
    const dataOffset = ru32(matchView, indexBase + 6);

    const sub = subs[subIndex]!;
    const nodeType = sub.nodeType;
    const node = readMdastMatchedNode(matchView, matchBuf, dataOffset, nodeId, nodeType, dirtyData);
    const result = sub.visitFn.call(plugin, node, context);

    if (result !== undefined && result !== null) {
      const cls = classifyReturn(result);
      switch (cls) {
        case "raw_markdown":
          returnBuffer.replace(nodeId, result as unknown as { raw: string });
          break;
        case "raw_html":
          returnBuffer.replace(nodeId, result as unknown as { rawHtml: string });
          break;
        case "structured_node": {
          const r = result as Record<string, unknown>;
          if ("children" in r && isChildRefArray(r.children)) {
            // Children are opaque refs — tell Rust to keep the original children
            const { children: _, ...rest } = r;
            returnBuffer.replaceRawJson(nodeId, JSON.stringify({ ...rest, _keepChildren: true }));
          } else {
            returnBuffer.replace(nodeId, result as MdastNode);
          }
          break;
        }
      }
    }
  }

  plugin.after?.(context);

  // Flush dirty node data to the Rust arena
  for (const [id, value] of dirtyData) {
    const json = value ? JSON.stringify(value) : "";
    setNodeData(handle, id, encoder.encode(json));
  }

  const { merged, hasMutations } = mergeAndReset(returnBuffer, context);
  return {
    commandBuffer: merged,
    diagnostics: context.getDiagnostics(),
    hasMutations,
  };
}
