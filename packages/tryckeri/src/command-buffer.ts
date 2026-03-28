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
// Field IDs — must match crates/mdast/src/commands.rs
// ---------------------------------------------------------------------------

export const FIELD_DEPTH = 0x0001;

export const FIELD_URL = 0x0010;
export const FIELD_TITLE = 0x0011;

export const FIELD_LANG = 0x0020;
export const FIELD_META = 0x0021;
export const FIELD_VALUE = 0x0022;

export const FIELD_ALT = 0x0030;

export const FIELD_ORDERED = 0x0040;
export const FIELD_START = 0x0041;
export const FIELD_SPREAD = 0x0042;

export const FIELD_CHECKED = 0x0050;

export const FIELD_IDENTIFIER = 0x0060;
export const FIELD_LABEL = 0x0061;
export const FIELD_REFERENCE_TYPE = 0x0062;

export const FIELD_NAME = 0x0070;

// ---------------------------------------------------------------------------
// Command bytes (0x01–0x0F)
// ---------------------------------------------------------------------------

export const CMD_REMOVE = 0x01;
export const CMD_SET_INT = 0x02;
export const CMD_SET_STRING = 0x03;
export const CMD_SET_BOOL = 0x04;
export const CMD_INSERT_BEFORE = 0x05;
export const CMD_INSERT_AFTER = 0x06;
export const CMD_PREPEND_CHILD = 0x07;
export const CMD_APPEND_CHILD = 0x08;
export const CMD_WRAP = 0x09;
export const CMD_SET_NULL = 0x0a;
export const CMD_REPLACE = 0x0b;

// ---------------------------------------------------------------------------
// Payload types (0x10+, distinct range from commands)
// ---------------------------------------------------------------------------

export const PAYLOAD_RAW_MARKDOWN = 0x10;
export const PAYLOAD_RAW_HTML = 0x11;
export const PAYLOAD_SERDE_JSON = 0x12;

// ---------------------------------------------------------------------------
// Property name → field ID mapping per node type
// ---------------------------------------------------------------------------

const FIELD_MAP: Record<string, Record<string, number>> = {
  heading: { depth: FIELD_DEPTH },
  link: { url: FIELD_URL, title: FIELD_TITLE },
  image: { url: FIELD_URL, alt: FIELD_ALT, title: FIELD_TITLE },
  code: { lang: FIELD_LANG, meta: FIELD_META, value: FIELD_VALUE },
  math: { meta: FIELD_META, value: FIELD_VALUE },
  text: { value: FIELD_VALUE },
  inlineCode: { value: FIELD_VALUE },
  html: { value: FIELD_VALUE },
  yaml: { value: FIELD_VALUE },
  toml: { value: FIELD_VALUE },
  inlineMath: { value: FIELD_VALUE },
  list: { ordered: FIELD_ORDERED, start: FIELD_START, spread: FIELD_SPREAD },
  listItem: { checked: FIELD_CHECKED, spread: FIELD_SPREAD },
  definition: {
    url: FIELD_URL,
    title: FIELD_TITLE,
    identifier: FIELD_IDENTIFIER,
    label: FIELD_LABEL,
  },
  linkReference: {
    identifier: FIELD_IDENTIFIER,
    label: FIELD_LABEL,
    referenceType: FIELD_REFERENCE_TYPE,
  },
  imageReference: {
    identifier: FIELD_IDENTIFIER,
    label: FIELD_LABEL,
    referenceType: FIELD_REFERENCE_TYPE,
  },
  footnoteReference: {
    identifier: FIELD_IDENTIFIER,
    label: FIELD_LABEL,
    referenceType: FIELD_REFERENCE_TYPE,
  },
  footnoteDefinition: { identifier: FIELD_IDENTIFIER, label: FIELD_LABEL },
  mdxJsxFlowElement: { name: FIELD_NAME },
  mdxJsxTextElement: { name: FIELD_NAME },
  mdxFlowExpression: { value: FIELD_VALUE },
  mdxTextExpression: { value: FIELD_VALUE },
  mdxjsEsm: { value: FIELD_VALUE },
};

// ---------------------------------------------------------------------------
// Return value classification
// ---------------------------------------------------------------------------

export type ReturnClass = "no_change" | "raw_markdown" | "raw_html" | "structured_node";

export function classifyReturn(value: unknown): ReturnClass {
  if (value === undefined || value === null) return "no_change";
  const v = value as Record<string, unknown>;
  if (typeof v.raw === "string") return "raw_markdown";
  if (typeof v.rawHtml === "string") return "raw_html";
  if (typeof v.type === "string") return "structured_node";
  throw new Error("Invalid return value from visitor: must have raw, rawHtml, or type");
}

/**
 * Resolve a property name to its field ID for a given node type.
 * Returns undefined if the property is not a known field for the node type.
 */
export function resolveFieldId(nodeType: string, propertyName: string): number | undefined {
  return FIELD_MAP[nodeType]?.[propertyName];
}

// ---------------------------------------------------------------------------
// CommandBuffer
// ---------------------------------------------------------------------------

const INITIAL_SIZE = 4096;
const encoder = new TextEncoder();

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

  setProperty(nodeType: string, nodeId: number, key: string, value: unknown): void {
    const fieldId = resolveFieldId(nodeType, key);
    if (fieldId === undefined) {
      throw new Error(`Unknown field "${key}" for node type "${nodeType}"`);
    }

    if (value === null || value === undefined) {
      this.ensureCapacity(7); // 1 + 4 + 2
      this.writeU8(CMD_SET_NULL);
      this.writeU32(nodeId);
      this.writeU16(fieldId);
    } else if (typeof value === "boolean") {
      this.ensureCapacity(8); // 1 + 4 + 2 + 1
      this.writeU8(CMD_SET_BOOL);
      this.writeU32(nodeId);
      this.writeU16(fieldId);
      this.writeU8(value ? 1 : 0);
    } else if (typeof value === "number") {
      this.ensureCapacity(15); // 1 + 4 + 2 + 8
      this.writeU8(CMD_SET_INT);
      this.writeU32(nodeId);
      this.writeU16(fieldId);
      this.writeI64(value);
    } else if (typeof value === "string") {
      const encoded = encoder.encode(value);
      this.ensureCapacity(11 + encoded.length); // 1 + 4 + 2 + 4 + len
      this.writeU8(CMD_SET_STRING);
      this.writeU32(nodeId);
      this.writeU16(fieldId);
      this.writeU32(encoded.length);
      this.writeBytes(encoded);
    } else {
      throw new Error(`Unsupported value type for setProperty: ${typeof value} (field "${key}")`);
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
    const encoded = encoder.encode(json);
    this.ensureCapacity(10 + encoded.length);
    this.writeU8(CMD_REPLACE);
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

  /** Reset the buffer for reuse. */
  reset(): void {
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

  private writeU16(val: number): void {
    this.view.setUint16(this.offset, val, true); // little-endian
    this.offset += 2;
  }

  private writeU32(val: number): void {
    this.view.setUint32(this.offset, val, true);
    this.offset += 4;
  }

  private writeI64(val: number): void {
    // Write as two 32-bit halves (little-endian). This handles values up to
    // Number.MAX_SAFE_INTEGER correctly. For the fields we care about (depth,
    // start, checked) the values are always small positive integers.
    this.view.setInt32(this.offset, val | 0, true);
    this.view.setInt32(this.offset + 4, val < 0 ? -1 : 0, true);
    this.offset += 8;
  }

  private writeBytes(data: Uint8Array): void {
    this.bytes.set(data, this.offset);
    this.offset += data.length;
  }
}
