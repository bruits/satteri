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

import { ByteWriter } from "./byte-writer.js";
import type { MdastNode } from "./types.js";
import {
  PROP_STRING,
  PROP_BOOL_TRUE,
  PROP_BOOL_FALSE,
  PROP_SPACE_SEP,
  PROP_INT,
  PROP_NULL,
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
  CMD_SET_CHILDREN,
  PAYLOAD_RAW_MARKDOWN,
  PAYLOAD_RAW_HTML,
  PAYLOAD_SERDE_JSON,
  PAYLOAD_OPSTREAM,
} from "./generated/wire-constants.js";

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

/** Structural commands that carry a subtree payload, named after the JSON-path
 *  method; `${op}Opstream` / `${op}RawJson` are the binary/raw twins. */
export type StructuralOp =
  | "replace"
  | "insertBefore"
  | "insertAfter"
  | "prependChild"
  | "appendChild"
  | "wrapNode";

export class CommandBuffer extends ByteWriter {
  constructor() {
    super(INITIAL_SIZE);
  }

  #u32(v: number): void {
    const buf = this.buf;
    let n = this.n;
    buf[n++] = v & 255;
    buf[n++] = (v >> 8) & 255;
    buf[n++] = (v >> 16) & 255;
    buf[n++] = (v >>> 24) & 255;
    this.n = n;
  }

  removeNode(nodeId: number): void {
    this.ensure(5);
    this.buf[this.n++] = CMD_REMOVE;
    this.#u32(nodeId);
  }

  /** Unified set-property for both MDAST and HAST nodes. */
  setProperty(nodeId: number, key: string, value: unknown): void {
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
      str = (value as string[]).join(" ");
    } else {
      valueType = PROP_STRING;
      str = String(value);
    }

    // 1(cmd) + 4(nodeId) + 1(valueType); name and value are length-prefixed strings
    this.ensure(6);
    this.buf[this.n++] = CMD_SET_PROPERTY;
    this.#u32(nodeId);
    this.buf[this.n++] = valueType;
    this.utf8WithU32Len(key);
    this.utf8WithU32Len(str);
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

  replaceRawJson(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_REPLACE, nodeId, json);
  }

  /** Replace a node's child list (Root-wrapped `json`) while keeping the node. */
  setChildren(nodeId: number, json: string): void {
    this.writeRawJsonCommand(CMD_SET_CHILDREN, nodeId, json);
  }

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
    this.writePayloadCommand(cmd, nodeId, PAYLOAD_SERDE_JSON, json);
  }

  /** Header (cmd + nodeId + payloadType) followed by a length-prefixed string payload. */
  private writePayloadCommand(cmd: number, nodeId: number, payloadType: number, s: string): void {
    this.ensure(6);
    this.buf[this.n++] = cmd;
    this.#u32(nodeId);
    this.buf[this.n++] = payloadType;
    this.utf8WithU32Len(s);
  }

  replaceOpstream(nodeId: number, ops: Uint8Array): void {
    this.writeOpstreamCommand(CMD_REPLACE, nodeId, ops);
  }

  /** Replace a node's child list (root-wrapped `ops`) while keeping the node. */
  setChildrenOpstream(nodeId: number, ops: Uint8Array): void {
    this.writeOpstreamCommand(CMD_SET_CHILDREN, nodeId, ops);
  }

  insertBeforeOpstream(nodeId: number, ops: Uint8Array): void {
    this.writeOpstreamCommand(CMD_INSERT_BEFORE, nodeId, ops);
  }

  insertAfterOpstream(nodeId: number, ops: Uint8Array): void {
    this.writeOpstreamCommand(CMD_INSERT_AFTER, nodeId, ops);
  }

  prependChildOpstream(nodeId: number, ops: Uint8Array): void {
    this.writeOpstreamCommand(CMD_PREPEND_CHILD, nodeId, ops);
  }

  appendChildOpstream(nodeId: number, ops: Uint8Array): void {
    this.writeOpstreamCommand(CMD_APPEND_CHILD, nodeId, ops);
  }

  wrapNodeOpstream(nodeId: number, ops: Uint8Array): void {
    this.writeOpstreamCommand(CMD_WRAP, nodeId, ops);
  }

  private writeOpstreamCommand(cmd: number, nodeId: number, ops: Uint8Array): void {
    this.ensure(10 + ops.length);
    this.buf[this.n++] = cmd;
    this.#u32(nodeId);
    this.buf[this.n++] = PAYLOAD_OPSTREAM;
    this.#u32(ops.length);
    this.buf.set(ops, this.n);
    this.n += ops.length;
  }

  /** Return a Uint8Array view of the written bytes (no copy). */
  getBuffer(): Uint8Array {
    return this.take();
  }

  /** Reset for reuse, releasing the old buffer (handed-out views stay intact). */
  override reset(): void {
    if (this.n === 0) return;
    this.release();
  }

  private writeStructuralCommand(cmd: number, nodeId: number, node: unknown): void {
    const v = node as Record<string, unknown>;
    if (typeof v.raw === "string") {
      this.writePayloadCommand(cmd, nodeId, PAYLOAD_RAW_MARKDOWN, v.raw as string);
    } else if (typeof v.rawHtml === "string") {
      this.writePayloadCommand(cmd, nodeId, PAYLOAD_RAW_HTML, v.rawHtml as string);
    } else {
      this.writePayloadCommand(cmd, nodeId, PAYLOAD_SERDE_JSON, JSON.stringify(node));
    }
  }
}
