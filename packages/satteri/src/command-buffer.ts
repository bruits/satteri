/**
 * Binary command buffer for efficient JS→Rust mutation serialization.
 *
 * Simple mutations (remove, setProperty) are encoded as compact binary commands.
 * Structural mutations (insert, replace) carry payloads that can be raw strings
 * (for Rust to re-parse) or JSON-serialized node trees.
 *
 * All multi-byte integers are little-endian to match native x86/ARM layout and
 * avoid byte-swapping on the Rust side.
 */

import type { MdastNode } from "./types.js";

// Command bytes (0x01–0x0F)

const CMD_REMOVE = 0x01;
const CMD_INSERT_BEFORE = 0x05;
const CMD_INSERT_AFTER = 0x06;
const CMD_PREPEND_CHILD = 0x07;
const CMD_APPEND_CHILD = 0x08;
const CMD_WRAP = 0x09;
const CMD_REPLACE = 0x0b;
const CMD_SET_PROPERTY = 0x0c;

// Payload types (0x10+, distinct range from commands)

const PAYLOAD_RAW_MARKDOWN = 0x10;
const PAYLOAD_RAW_HTML = 0x11;
const PAYLOAD_SERDE_JSON = 0x12;

// Value types for CMD_SET_PROPERTY (must match commands.rs PROP_* constants)

const PROP_STRING = 0;
const PROP_BOOL_TRUE = 1;
const PROP_BOOL_FALSE = 2;
const PROP_SPACE_SEP = 3;
const PROP_INT = 5;
const PROP_NULL = 6;

type ReturnClass = "no_change" | "raw_markdown" | "raw_html" | "structured_node";

export function classifyReturn(value: unknown): ReturnClass {
  if (value === undefined || value === null) return "no_change";
  const v = value as Record<string, unknown>;
  if (typeof v.raw === "string") return "raw_markdown";
  if (typeof v.rawHtml === "string") return "raw_html";
  if (typeof v.type === "string") return "structured_node";
  throw new Error("Invalid return value from visitor: must have raw, rawHtml, or type");
}

const INITIAL_SIZE = 4096;
const encoder = new TextEncoder();

/** Module-level free list for `CommandBuffer` instances.
 *
 *  Each plugin pass on a small fixture allocates *two* CommandBuffers — one
 *  for the visitor context's mutation commands and one for the return-value
 *  replacements. At ~4 KB ArrayBuffer per instance, plugin-heavy workloads
 *  burn substantial GC time on what is structurally a reusable buffer. The
 *  freelist below recycles instances between visits; sites that finish with a
 *  buffer call `releaseCommandBuffer(buf)` and `acquireCommandBuffer()` returns
 *  one that's already been `reset()` (offset=0, same ArrayBuffer). Cap keeps
 *  the pool bounded for long-lived processes that briefly burst high. */
const COMMAND_BUFFER_POOL_MAX = 8;
const commandBufferPool: CommandBuffer[] = [];

export function acquireCommandBuffer(): CommandBuffer {
  const pooled = commandBufferPool.pop();
  if (pooled !== undefined) return pooled;
  return new CommandBuffer();
}

export function releaseCommandBuffer(buf: CommandBuffer): void {
  if (commandBufferPool.length >= COMMAND_BUFFER_POOL_MAX) return;
  buf.reset();
  commandBufferPool.push(buf);
}

export class CommandBuffer {
  private buffer: ArrayBuffer;
  private view: DataView;
  private bytes: Uint8Array;
  private offset: number = 0;

  constructor() {
    this.buffer = new ArrayBuffer(INITIAL_SIZE);
    this.view = new DataView(this.buffer);
    this.bytes = new Uint8Array(this.buffer);
  }

  removeNode(nodeId: number): void {
    this.ensureCapacity(5);
    this.writeU8(CMD_REMOVE);
    this.writeU32(nodeId);
  }

  /** Unified set-property for both MDAST and HAST nodes.
   *
   *  Hot path: profiles show plugins that touch every node spend ~38% of
   *  total time inside `TextEncoder.encode` — almost entirely on short
   *  property keys/values. `encodeInto` writes UTF-8 straight into the
   *  command buffer, no per-call `Uint8Array` allocation. We reserve the
   *  worst-case UTF-8 length (`str.length * 3`) up front and backfill the
   *  length prefix once the actual byte count is known. */
  setProperty(nodeId: number, key: string, value: unknown): void {
    let valueType: number;
    let valueStr: string | null;

    if (value === null || value === undefined) {
      valueType = PROP_NULL;
      valueStr = null;
    } else if (value === true) {
      valueType = PROP_BOOL_TRUE;
      valueStr = null;
    } else if (value === false) {
      valueType = PROP_BOOL_FALSE;
      valueStr = null;
    } else if (typeof value === "number") {
      valueType = PROP_INT;
      valueStr = String(value);
    } else if (Array.isArray(value)) {
      valueType = PROP_SPACE_SEP;
      valueStr = (value as string[]).join(" ");
    } else {
      valueType = PROP_STRING;
      valueStr = String(value);
    }

    // 1(cmd) + 4(nodeId) + 1(valueType) + 4(nameLen) + nameBytes(≤3*key.length)
    //   + 4(valueLen) + valueBytes(≤3*valueStr.length)
    const maxNameLen = key.length * 3;
    const maxValueLen = valueStr === null ? 0 : valueStr.length * 3;
    this.ensureCapacity(14 + maxNameLen + maxValueLen);
    this.writeU8(CMD_SET_PROPERTY);
    this.writeU32(nodeId);
    this.writeU8(valueType);
    this.writeStringWithLen(key);
    if (valueStr === null) {
      this.writeU32(0);
    } else {
      this.writeStringWithLen(valueStr);
    }
  }

  insertBefore(nodeId: number, newNode: MdastNode | { raw: string } | { rawHtml: string }): void {
    this.writeStructuralCommand(CMD_INSERT_BEFORE, nodeId, newNode);
  }

  insertAfter(nodeId: number, newNode: MdastNode | { raw: string } | { rawHtml: string }): void {
    this.writeStructuralCommand(CMD_INSERT_AFTER, nodeId, newNode);
  }

  prependChild(nodeId: number, newNode: MdastNode | { raw: string } | { rawHtml: string }): void {
    this.writeStructuralCommand(CMD_PREPEND_CHILD, nodeId, newNode);
  }

  appendChild(nodeId: number, newNode: MdastNode | { raw: string } | { rawHtml: string }): void {
    this.writeStructuralCommand(CMD_APPEND_CHILD, nodeId, newNode);
  }

  wrapNode(nodeId: number, parentNode: MdastNode | { raw: string } | { rawHtml: string }): void {
    this.writeStructuralCommand(CMD_WRAP, nodeId, parentNode);
  }

  replace(nodeId: number, newNode: MdastNode | { raw: string } | { rawHtml: string }): void {
    this.writeStructuralCommand(CMD_REPLACE, nodeId, newNode);
  }

  /** Write a REPLACE command with a pre-serialized JSON payload. */
  replaceRawJson(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_REPLACE, nodeId, json);
  }

  /** Write any structural command with a pre-serialized JSON payload. */
  insertBeforeRawJson(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_INSERT_BEFORE, nodeId, json);
  }

  insertAfterRawJson(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_INSERT_AFTER, nodeId, json);
  }

  prependChildRawJson(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_PREPEND_CHILD, nodeId, json);
  }

  appendChildRawJson(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_APPEND_CHILD, nodeId, json);
  }

  wrapNodeRawJson(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_WRAP, nodeId, json);
  }

  private writeRawJsonCommand(cmd: number, nodeId: number, json: string): void {
    this.ensureCapacity(10 + json.length * 3);
    this.writeU8(cmd);
    this.writeU32(nodeId);
    this.writeU8(PAYLOAD_SERDE_JSON);
    this.writeStringWithLen(json);
  }

  /** Return a Uint8Array view of the written bytes (no copy). */
  getBuffer(): Uint8Array {
    return new Uint8Array(this.buffer, 0, this.offset);
  }

  /** Number of bytes written so far. */
  get length(): number {
    return this.offset;
  }

  /** Reset the write cursor for reuse. The backing ArrayBuffer is intentionally
   *  kept — the next write overwrites in place, avoiding a fresh 4 KB alloc
   *  per compile. Safe because `getBuffer()` returns a view over the same
   *  ArrayBuffer; callers must consume that view before the next write. */
  reset(): void {
    this.offset = 0;
  }

  private writeStructuralCommand(cmd: number, nodeId: number, node: unknown): void {
    const v = node as Record<string, unknown>;
    if (typeof v.raw === "string") {
      const raw = v.raw as string;
      // 1(cmd) + 4(nodeId) + 1(payloadType) + 4(len) + payload(≤3*raw.length)
      this.ensureCapacity(10 + raw.length * 3);
      this.writeU8(cmd);
      this.writeU32(nodeId);
      this.writeU8(PAYLOAD_RAW_MARKDOWN);
      this.writeStringWithLen(raw);
    } else if (typeof v.rawHtml === "string") {
      const rawHtml = v.rawHtml as string;
      this.ensureCapacity(10 + rawHtml.length * 3);
      this.writeU8(cmd);
      this.writeU32(nodeId);
      this.writeU8(PAYLOAD_RAW_HTML);
      this.writeStringWithLen(rawHtml);
    } else {
      // Structured node, serialize as JSON
      const json = JSON.stringify(node);
      this.ensureCapacity(10 + json.length * 3);
      this.writeU8(cmd);
      this.writeU32(nodeId);
      this.writeU8(PAYLOAD_SERDE_JSON);
      this.writeStringWithLen(json);
    }
  }

  private ensureCapacity(needed: number): void {
    while (this.offset + needed > this.buffer.byteLength) {
      this.grow();
    }
  }

  private grow(): void {
    const newBuffer = new ArrayBuffer(this.buffer.byteLength * 2);
    new Uint8Array(newBuffer).set(this.bytes);
    this.buffer = newBuffer;
    this.view = new DataView(this.buffer);
    this.bytes = new Uint8Array(this.buffer);
  }

  private writeU8(val: number): void {
    this.view.setUint8(this.offset, val);
    this.offset += 1;
  }

  private writeU32(val: number): void {
    this.view.setUint32(this.offset, val, true);
    this.offset += 4;
  }

  private writeBytes(data: Uint8Array): void {
    this.bytes.set(data, this.offset);
    this.offset += data.length;
  }

  /** Encode `str` as UTF-8 directly into the buffer, prefixed by a u32 byte
   *  length. Caller must `ensureCapacity(4 + str.length * 3)` beforehand so
   *  the worst-case UTF-8 expansion fits. Uses `encodeInto` to avoid the
   *  intermediate `Uint8Array` that `TextEncoder.encode` would allocate. */
  private writeStringWithLen(str: string): void {
    const lenOffset = this.offset;
    this.offset += 4;
    const start = this.offset;
    const { written } = encoder.encodeInto(str, this.bytes.subarray(start));
    this.view.setUint32(lenOffset, written, true);
    this.offset = start + written;
  }
}
