// Little-endian fields match the Rust wire protocol without byte swapping on x86/ARM.

import { OpWriter } from "./op-stream.js";
import type { MdastNode } from "./types.js";
import {
  PROP_STRING,
  PROP_BOOL_TRUE,
  PROP_BOOL_FALSE,
  PROP_SPACE_SEP,
  PROP_INT,
  PROP_NULL,
  PROP_TOKEN_LIST,
} from "./op-stream.js";
import {
  CMD_REMOVE,
  CMD_INSERT_BEFORE,
  CMD_INSERT_AFTER,
  CMD_PREPEND_CHILD,
  CMD_APPEND_CHILD,
  CMD_WRAP,
  CMD_REPLACE,
  CMD_SET_PROPERTY,
  CMD_SET_FIELD,
  CMD_SET_ATTRIBUTE,
  PAYLOAD_RAW,
  RAW_LITERAL_BRACES,
  PAYLOAD_OPSTREAM,
  PAYLOAD_SEQUENCE,
  PROP_EXPRESSION,
} from "./generated/wire-constants.js";

type ReturnClass = "no_change" | "raw_markdown" | "raw_html" | "structured_node";

type StructuralContent =
  | MdastNode
  | { raw: string; mdxExpressions?: boolean }
  | { rawHtml: string };

export function classifyReturn(value: unknown): ReturnClass {
  if (value === undefined || value === null) return "no_change";
  const v = value as Record<string, unknown>;
  if (typeof v.raw === "string") return "raw_markdown";
  if (typeof v.rawHtml === "string") return "raw_html";
  if (typeof v.type === "string") return "structured_node";
  throw new Error("Invalid return value from visitor: must have raw, rawHtml, or type");
}

const INITIAL_SIZE = 4096;

// takeCommands copies bytes before release, so pooled buffers cannot invalidate returned commands.
const COMMAND_BUFFER_POOL_MAX = 8;
/** Backings above this are dropped on release so one huge compile does not retain megabytes forever. */
export const COMMAND_BUFFER_RETAIN_MAX = 1 << 20;
const commandBufferPool: CommandBuffer[] = [];

export function acquireCommandBuffer(): CommandBuffer {
  return commandBufferPool.pop() ?? new CommandBuffer();
}

export function releaseCommandBuffer(buf: CommandBuffer): void {
  if (commandBufferPool.length >= COMMAND_BUFFER_POOL_MAX) return;
  if (buf.capacity > COMMAND_BUFFER_RETAIN_MAX) return;
  buf.reset();
  commandBufferPool.push(buf);
}

export type StructuralOp =
  | "replace"
  | "insertBefore"
  | "insertAfter"
  | "prependChild"
  | "appendChild"
  | "wrapNode";

export const STRUCTURAL_CMD: Record<StructuralOp, number> = {
  replace: CMD_REPLACE,
  insertBefore: CMD_INSERT_BEFORE,
  insertAfter: CMD_INSERT_AFTER,
  prependChild: CMD_PREPEND_CHILD,
  appendChild: CMD_APPEND_CHILD,
  wrapNode: CMD_WRAP,
};

export class CommandBuffer extends OpWriter {
  // Commands must not interleave bytes while a structural payload is being emitted.
  #inOpstream = false;
  constructor() {
    super(INITIAL_SIZE);
  }

  override reset(): void {
    this.#inOpstream = false;
    super.reset();
  }

  #assertNotEncoding(): void {
    if (this.#inOpstream) {
      throw new Error(
        "reentrant command emission: a node getter or toJSON invoked a context mutation while a structural payload was being encoded",
      );
    }
  }

  // Roll back failed payloads so the Rust decoder never sees a partial command.
  emitOpstreamCommand(cmd: number, nodeId: number, emit: () => void): void {
    const commandStart = this.n;
    const lenPos = this.#beginOpstream(cmd, nodeId);
    try {
      emit();
    } catch (error) {
      this.#abortOpstream(commandStart);
      throw error;
    }
    this.#endOpstream(lenPos);
  }

  #beginOpstream(cmd: number, nodeId: number): number {
    this.#assertNotEncoding();
    this.#inOpstream = true;
    this.ensure(10);
    this.buf[this.n++] = cmd;
    this.writeU32(nodeId);
    this.buf[this.n++] = PAYLOAD_OPSTREAM;
    const lenPos = this.n;
    this.n += 4;
    return lenPos;
  }

  /** One structural command carrying `items` in order; raw items are parsed in place. */
  emitSequenceCommand(
    cmd: number,
    nodeId: number,
    items: readonly unknown[],
    emitNode: (item: unknown) => void,
  ): void {
    this.#assertNotEncoding();
    const commandStart = this.n;
    this.ensure(10);
    this.buf[this.n++] = cmd;
    this.writeU32(nodeId);
    this.buf[this.n++] = PAYLOAD_SEQUENCE;
    this.writeU32(items.length);
    for (const item of items) {
      if (this.#writeRawPayload(item)) continue;
      this.ensure(5);
      this.buf[this.n++] = PAYLOAD_OPSTREAM;
      const lenPos = this.n;
      this.n += 4;
      this.#inOpstream = true;
      try {
        emitNode(item);
      } catch (error) {
        this.#abortOpstream(commandStart);
        throw error;
      }
      this.#endOpstream(lenPos);
    }
  }

  #endOpstream(lenPos: number): void {
    this.#inOpstream = false;
    this.patchU32(lenPos, this.n - (lenPos + 4));
  }

  #abortOpstream(commandStart: number): void {
    this.#inOpstream = false;
    this.n = commandStart;
  }

  removeNode(nodeId: number): void {
    this.#assertNotEncoding();
    this.ensure(5);
    this.buf[this.n++] = CMD_REMOVE;
    this.writeU32(nodeId);
  }

  setProperty(nodeId: number, key: string, value: unknown): void {
    this.#writeNamedValue(CMD_SET_PROPERTY, nodeId, key, value);
  }

  setField(nodeId: number, key: string, value: unknown): void {
    this.#writeNamedValue(CMD_SET_FIELD, nodeId, key, value);
  }

  /** `value` is checked by the caller, so an object here is an MDX expression. */
  setAttribute(nodeId: number, name: string, value: unknown): void {
    if (value !== null && typeof value === "object") {
      const source = (value as { value: string }).value;
      this.#writeNamed(CMD_SET_ATTRIBUTE, nodeId, PROP_EXPRESSION, name, source);
      return;
    }
    this.#writeNamedValue(CMD_SET_ATTRIBUTE, nodeId, name, value);
  }

  #writeNamedValue(cmd: number, nodeId: number, key: string, value: unknown): void {
    let valueType: number;
    let str: string;

    if (value === null || value === undefined) {
      valueType = PROP_NULL;
      str = "";
    } else if (value === true) {
      valueType = PROP_BOOL_TRUE;
      str = "";
    } else if (value === false) {
      valueType = PROP_BOOL_FALSE;
      str = "";
    } else if (typeof value === "number") {
      valueType = PROP_INT;
      str = String(value);
    } else if (Array.isArray(value)) {
      valueType = PROP_SPACE_SEP;
      str = (value as unknown[]).join(" ");
    } else {
      valueType = PROP_STRING;
      str = String(value);
    }
    this.#writeNamed(cmd, nodeId, valueType, key, str);
  }

  #writeNamed(cmd: number, nodeId: number, valueType: number, key: string, str: string): void {
    this.#assertNotEncoding();
    this.ensure(6);
    this.buf[this.n++] = cmd;
    this.writeU32(nodeId);
    this.buf[this.n++] = valueType;
    this.utf8WithU32Len(key);
    this.utf8WithU32Len(str);
  }

  /** Set a HAST element's list property from `encodeTokenList` bytes, whose
   *  separator the renderer picks from the element's schema. */
  setTokenListProperty(nodeId: number, key: string, tokens: string): void {
    this.#assertNotEncoding();
    this.ensure(6);
    this.buf[this.n++] = CMD_SET_PROPERTY;
    this.writeU32(nodeId);
    this.buf[this.n++] = PROP_TOKEN_LIST;
    this.utf8WithU32Len(key);
    this.utf8WithU32Len(tokens);
  }

  insertBefore(nodeId: number, newNode: StructuralContent): void {
    this.writeStructuralCommand(CMD_INSERT_BEFORE, nodeId, newNode);
  }

  insertAfter(nodeId: number, newNode: StructuralContent): void {
    this.writeStructuralCommand(CMD_INSERT_AFTER, nodeId, newNode);
  }

  prependChild(nodeId: number, newNode: StructuralContent): void {
    this.writeStructuralCommand(CMD_PREPEND_CHILD, nodeId, newNode);
  }

  appendChild(nodeId: number, newNode: StructuralContent): void {
    this.writeStructuralCommand(CMD_APPEND_CHILD, nodeId, newNode);
  }

  wrapNode(nodeId: number, parentNode: StructuralContent): void {
    this.writeStructuralCommand(CMD_WRAP, nodeId, parentNode);
  }

  replace(nodeId: number, newNode: StructuralContent): void {
    this.writeStructuralCommand(CMD_REPLACE, nodeId, newNode);
  }

  #writeRawPayload(node: unknown): boolean {
    const v = node as Record<string, unknown>;
    let flags: number;
    let raw: string;
    if (typeof v.raw === "string") {
      flags = v.mdxExpressions === false ? RAW_LITERAL_BRACES : 0;
      raw = v.raw;
    } else if (typeof v.rawHtml === "string") {
      flags = RAW_LITERAL_BRACES;
      raw = v.rawHtml;
    } else {
      return false;
    }
    this.ensure(2);
    this.buf[this.n++] = PAYLOAD_RAW;
    this.buf[this.n++] = flags;
    this.utf8WithU32Len(raw);
    return true;
  }

  // This view aliases the buffer and must be copied before it is released to the pool.
  getBuffer(): Uint8Array {
    return this.take();
  }

  private writeStructuralCommand(cmd: number, nodeId: number, node: unknown): void {
    this.#assertNotEncoding();
    const commandStart = this.n;
    this.ensure(5);
    this.buf[this.n++] = cmd;
    this.writeU32(nodeId);
    if (this.#writeRawPayload(node)) return;
    this.n = commandStart;
    throw new Error("CommandBuffer: structural content must be {raw} or {rawHtml}");
  }
}
