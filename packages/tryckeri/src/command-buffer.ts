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

// ---------------------------------------------------------------------------
// Command bytes (0x01–0x0F)
// ---------------------------------------------------------------------------

const CMD_REMOVE = 0x01;
const CMD_INSERT_BEFORE = 0x05;
const CMD_INSERT_AFTER = 0x06;
const CMD_PREPEND_CHILD = 0x07;
const CMD_APPEND_CHILD = 0x08;
const CMD_WRAP = 0x09;
const CMD_REPLACE = 0x0b;
const CMD_SET_PROPERTY = 0x0c;

// ---------------------------------------------------------------------------
// Payload types (0x10+, distinct range from commands)
// ---------------------------------------------------------------------------

const PAYLOAD_RAW_MARKDOWN = 0x10;
const PAYLOAD_RAW_HTML = 0x11;
const PAYLOAD_SERDE_JSON = 0x12;

// ---------------------------------------------------------------------------
// Value types for CMD_SET_PROPERTY (must match commands.rs PROP_* constants)
// ---------------------------------------------------------------------------

const PROP_STRING = 0;
const PROP_BOOL_TRUE = 1;
const PROP_BOOL_FALSE = 2;
const PROP_SPACE_SEP = 3;
const PROP_INT = 4;
const PROP_NULL = 5;

// ---------------------------------------------------------------------------
// Return value classification
// ---------------------------------------------------------------------------

type ReturnClass = "no_change" | "raw_markdown" | "raw_html" | "structured_node";

export function classifyReturn(value: unknown): ReturnClass {
  if (value === undefined || value === null) return "no_change";
  const v = value as Record<string, unknown>;
  if (typeof v.raw === "string") return "raw_markdown";
  if (typeof v.rawHtml === "string") return "raw_html";
  if (typeof v.type === "string") return "structured_node";
  throw new Error("Invalid return value from visitor: must have raw, rawHtml, or type");
}

// ---------------------------------------------------------------------------
// CommandBuffer
// ---------------------------------------------------------------------------

const INITIAL_SIZE = 4096;
const encoder = new TextEncoder();
const EMPTY_U8 = new Uint8Array(0);

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

  // -- Public command methods -----------------------------------------------

  removeNode(nodeId: number): void {
    this.ensureCapacity(5);
    this.writeU8(CMD_REMOVE);
    this.writeU32(nodeId);
  }

  /** Unified set-property for both MDAST and HAST nodes. */
  setProperty(nodeId: number, key: string, value: unknown): void {
    const encodedName = encoder.encode(key);
    let valueType: number;
    let encodedValue: Uint8Array;

    if (value === null || value === undefined) {
      valueType = PROP_NULL;
      encodedValue = EMPTY_U8;
    } else if (value === true) {
      valueType = PROP_BOOL_TRUE;
      encodedValue = EMPTY_U8;
    } else if (value === false) {
      valueType = PROP_BOOL_FALSE;
      encodedValue = EMPTY_U8;
    } else if (typeof value === "number") {
      valueType = PROP_INT;
      encodedValue = encoder.encode(String(value));
    } else if (Array.isArray(value)) {
      valueType = PROP_SPACE_SEP;
      encodedValue = encoder.encode((value as string[]).join(" "));
    } else {
      valueType = PROP_STRING;
      encodedValue = encoder.encode(String(value));
    }

    // 1(cmd) + 4(nodeId) + 1(valueType) + 4(nameLen) + name + 4(valueLen) + value
    this.ensureCapacity(14 + encodedName.length + encodedValue.length);
    this.writeU8(CMD_SET_PROPERTY);
    this.writeU32(nodeId);
    this.writeU8(valueType);
    this.writeU32(encodedName.length);
    this.writeBytes(encodedName);
    this.writeU32(encodedValue.length);
    this.writeBytes(encodedValue);
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
    const encoded = encoder.encode(json);
    this.ensureCapacity(10 + encoded.length);
    this.writeU8(cmd);
    this.writeU32(nodeId);
    this.writeU8(PAYLOAD_SERDE_JSON);
    this.writeU32(encoded.length);
    this.writeBytes(encoded);
  }

  // -- Result accessors -----------------------------------------------------

  /** Return a Uint8Array view of the written bytes (no copy). */
  getBuffer(): Uint8Array {
    return new Uint8Array(this.buffer, 0, this.offset);
  }

  /** Number of bytes written so far. */
  get length(): number {
    return this.offset;
  }

  /** Reset the buffer for reuse, releasing the old ArrayBuffer. */
  reset(): void {
    this.buffer = new ArrayBuffer(INITIAL_SIZE);
    this.view = new DataView(this.buffer);
    this.bytes = new Uint8Array(this.buffer);
    this.offset = 0;
  }

  // -- Private helpers ------------------------------------------------------

  private writeStructuralCommand(cmd: number, nodeId: number, node: unknown): void {
    const v = node as Record<string, unknown>;
    if (typeof v.raw === "string") {
      const encoded = encoder.encode(v.raw as string);
      this.ensureCapacity(10 + encoded.length); // 1(cmd) + 4(nodeId) + 1(payloadType) + 4(len) + payload
      this.writeU8(cmd);
      this.writeU32(nodeId);
      this.writeU8(PAYLOAD_RAW_MARKDOWN);
      this.writeU32(encoded.length);
      this.writeBytes(encoded);
    } else if (typeof v.rawHtml === "string") {
      const encoded = encoder.encode(v.rawHtml as string);
      this.ensureCapacity(10 + encoded.length);
      this.writeU8(cmd);
      this.writeU32(nodeId);
      this.writeU8(PAYLOAD_RAW_HTML);
      this.writeU32(encoded.length);
      this.writeBytes(encoded);
    } else {
      // Structured node — serialize as JSON
      const json = JSON.stringify(node);
      const encoded = encoder.encode(json);
      this.ensureCapacity(10 + encoded.length);
      this.writeU8(cmd);
      this.writeU32(nodeId);
      this.writeU8(PAYLOAD_SERDE_JSON);
      this.writeU32(encoded.length);
      this.writeBytes(encoded);
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
}
