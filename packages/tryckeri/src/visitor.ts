import { materializeNode, TYPE_NAMES } from "./materializer.js";
import { CommandBuffer, classifyReturn } from "./command-buffer.js";
import type { MdastNode, MaybePromise } from "./types.js";
import type { MdastReader } from "./mdast-reader.js";
import type { DataMap } from "./data-map.js";

export const MutationType = {
  Replace: "replace",
  Remove: "remove",
  InsertBefore: "insertBefore",
  InsertAfter: "insertAfter",
  Wrap: "wrap",
  PrependChild: "prependChild",
  AppendChild: "appendChild",
  SetProperty: "setProperty",
} as const;

export type MutationTypeValue = (typeof MutationType)[keyof typeof MutationType];

export interface Mutation {
  type: MutationTypeValue;
  nodeId: number;
  newNode?: MdastNode;
  key?: string;
  value?: unknown;
}

export interface Diagnostic {
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

export class VisitorContext {
  readonly #commandBuffer: CommandBuffer = new CommandBuffer();
  readonly #diagnostics: Diagnostic[] = [];
  readonly #reader: MdastReader;
  readonly #dataMap: DataMap;
  readonly #rootId: number = 0;

  constructor(reader: MdastReader, dataMap: DataMap) {
    this.#reader = reader;
    this.#dataMap = dataMap;
  }

  removeNode(node: MdastNode): void {
    this.#commandBuffer.removeNode(node._nodeId);
  }

  insertBefore(node: MdastNode, newNode: MdastNode): void {
    this.#commandBuffer.insertBefore(node._nodeId, newNode);
  }

  insertAfter(node: MdastNode, newNode: MdastNode): void {
    this.#commandBuffer.insertAfter(node._nodeId, newNode);
  }

  wrapNode(node: MdastNode, parentNode: MdastNode): void {
    this.#commandBuffer.wrapNode(node._nodeId, parentNode);
  }

  prependChild(node: MdastNode, childNode: MdastNode): void {
    this.#commandBuffer.prependChild(node._nodeId, childNode);
  }

  appendChild(node: MdastNode, childNode: MdastNode): void {
    this.#commandBuffer.appendChild(node._nodeId, childNode);
  }

  replaceNode(node: MdastNode, newNode: MdastNode): void {
    this.#commandBuffer.replace(node._nodeId, newNode);
  }

  setProperty(node: MdastNode, key: string, value: unknown): void {
    this.#commandBuffer.setProperty(node.type, node._nodeId, key, value);
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
      nodeId: node?._nodeId,
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

  getDiagnostics(): Diagnostic[] {
    return this.#diagnostics;
  }
}

export interface PluginInstance {
  before?(context: VisitorContext): MaybePromise<void>;
  after?(context: VisitorContext): MaybePromise<void>;
  transformRoot?(root: MdastNode, context: VisitorContext): MaybePromise<MdastNode | undefined | null>;
  [nodeTypeName: string]: unknown;
}

export interface VisitResult {
  /** Binary command buffer containing all mutations. */
  commandBuffer: Uint8Array;
  diagnostics: Diagnostic[];
  hasMutations: boolean;
}

/**
 * Walk the MDAST and dispatch to plugin visitor functions.
 *
 * Mutations are collected into a binary command buffer. Return values from
 * visitor functions are classified (raw/rawHtml/structured) and encoded
 * as REPLACE commands in the buffer.
 */
export async function visitMdast(
  reader: MdastReader,
  plugin: PluginInstance,
  dataMap: DataMap,
): Promise<VisitResult> {
  const context = new VisitorContext(reader, dataMap);

  {
    const v = plugin.before?.(context);
    if (v instanceof Promise) await v;
  }

  // Separate CommandBuffer for return-value mutations (replace commands from
  // visitor return values). These are merged with the context's buffer at the end.
  const returnBuffer = new CommandBuffer();

  if (typeof plugin.transformRoot === "function") {
    // Full materialization path
    const root = materializeNode(reader, 0, dataMap);
    let result = plugin.transformRoot(root, context);
    if (result instanceof Promise) result = await result;
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
      (node: MdastNode, context: VisitorContext) => unknown
    >();
    for (const [name, fn] of Object.entries(plugin)) {
      if (VISITOR_KEYS.has(name) && typeof fn === "function") {
        for (const [num, typeName] of Object.entries(TYPE_NAMES)) {
          if (typeName === name) {
            TYPE_TO_VISITOR.set(
              Number(num),
              fn as (node: MdastNode, context: VisitorContext) => unknown,
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
        let result = visitor.call(plugin, node, context);
        if (result instanceof Promise) result = await result;
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

      const childIds = reader.getChildIds(nodeId);
      for (let i = childIds.length - 1; i >= 0; i--) {
        stack.push(childIds[i]!);
      }
    }
  }

  {
    const v = plugin.after?.(context);
    if (v instanceof Promise) await v;
  }

  // Merge: return-value commands first, then context commands
  const ctxBuf = context.getCommandBuffer().getBuffer();
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

  return {
    commandBuffer: merged,
    diagnostics: context.getDiagnostics(),
    hasMutations: totalLen > 0,
  };
}
