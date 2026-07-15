/**
 * Binary command buffer for efficient JS→Rust mutation serialization.
 *
 * Simple mutations (remove, setProperty) are encoded as compact binary commands.
 * Structural mutations (insert, replace, …) carry one of two payload kinds:
 * compiled op-streams (`PAYLOAD_OPSTREAM`, replayed straight into the arena —
 * see op-stream.ts) for declarative content, or raw markdown/HTML strings
 * (re-parsed by Rust) for the `{raw}`/`{rawHtml}` escape hatches.
 *
 * All multi-byte integers are little-endian to match native x86/ARM layout and
 * avoid byte-swapping on the Rust side.
 */

import { OpWriter } from "./op-stream.js";
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
  PAYLOAD_OPSTREAM,
} from "./generated/wire-constants.js";

type ReturnClass = "no_change" | "raw_markdown" | "raw_html" | "structured_node";

/** Content accepted by the structural mutators: a declarative node, a raw string
 *  re-parsed as Markdown (`mdxExpressions: false` keeps MDX `{…}` literal), or
 *  the deprecated `{rawHtml}` alias. */
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

/** Free list recycling `CommandBuffer` instances (and their grown backings)
 *  across plugin passes. Safe to retain the backing because `mergeAndReset`
 *  copies the bytes out before a buffer is released, so no view outlives a
 *  pass. Cap bounds the pool for processes that briefly burst high. */
const COMMAND_BUFFER_POOL_MAX = 8;
const commandBufferPool: CommandBuffer[] = [];

export function acquireCommandBuffer(): CommandBuffer {
  return commandBufferPool.pop() ?? new CommandBuffer();
}

export function releaseCommandBuffer(buf: CommandBuffer): void {
  if (commandBufferPool.length >= COMMAND_BUFFER_POOL_MAX) return;
  buf.reset();
  commandBufferPool.push(buf);
}

/** Structural commands that carry a subtree payload emitted in place via `emitOpstreamCommand`. */
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
  /** Set while a structural payload is being emitted in place; command
   *  methods must not interleave bytes into it. */
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

  /** Emit a structural command whose opstream payload is written by `emit`
   *  via the inherited op methods. If `emit` returns false or throws, the
   *  buffer is rolled back to the command start so the Rust decoder never
   *  sees a half-written command. Returns `emit`'s verdict. */
  emitOpstreamCommand(cmd: number, nodeId: number, emit: () => boolean): boolean {
    const commandStart = this.n;
    const lenPos = this.#beginOpstream(cmd, nodeId);
    // ok starts false so a throwing emit still hits the abort in finally
    let ok = false;
    try {
      ok = emit();
    } finally {
      if (!ok) this.#abortOpstream(commandStart);
    }
    if (ok) this.#endOpstream(lenPos);
    return ok;
  }

  /** Open a structural command whose opstream payload is emitted in place via
   *  the inherited op methods; returns the backpatch position for
   *  `#endOpstream`. Pair with `#abortOpstream` on failure. */
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

  #endOpstream(lenPos: number): void {
    this.#inOpstream = false;
    this.patchU32(lenPos, this.n - (lenPos + 4));
  }

  /** Roll back an in-progress opstream command (unencodable content). */
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

  /** Unified set-property for both MDAST and HAST nodes.
   *
   *  Hot path: uses `encodeInto` to write UTF-8 straight into the buffer (no
   *  per-call `Uint8Array`), reserving the worst-case length up front and
   *  backfilling the length prefix once the byte count is known. */
  setProperty(nodeId: number, key: string, value: unknown): void {
    this.#assertNotEncoding();
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
    this.writeU32(nodeId);
    this.buf[this.n++] = valueType;
    this.utf8WithU32Len(key);
    this.utf8WithU32Len(str);
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

  /** Header (cmd + nodeId + payloadType) followed by a length-prefixed string payload. */
  private writePayloadCommand(cmd: number, nodeId: number, payloadType: number, s: string): void {
    this.#assertNotEncoding();
    this.ensure(6);
    this.buf[this.n++] = cmd;
    this.writeU32(nodeId);
    this.buf[this.n++] = payloadType;
    this.utf8WithU32Len(s);
  }

  /** Return a Uint8Array view of the written bytes (no copy). */
  getBuffer(): Uint8Array {
    return this.take();
  }

  /** Raw structural content escape hatch. The string is re-parsed as Markdown;
   *  `mdxExpressions: false` (or the deprecated `{rawHtml}` alias) instead routes
   *  through the brace-escaping payload so MDX `{…}` stay literal. Declarative
   *  nodes go through the `*Opstream` methods, not here. */
  private writeStructuralCommand(cmd: number, nodeId: number, node: unknown): void {
    const v = node as Record<string, unknown>;
    if (typeof v.raw === "string") {
      const payload = v.mdxExpressions === false ? PAYLOAD_RAW_HTML : PAYLOAD_RAW_MARKDOWN;
      this.writePayloadCommand(cmd, nodeId, payload, v.raw as string);
    } else if (typeof v.rawHtml === "string") {
      this.writePayloadCommand(cmd, nodeId, PAYLOAD_RAW_HTML, v.rawHtml as string);
    } else {
      throw new Error("CommandBuffer: structural content must be {raw} or {rawHtml}");
    }
  }
}
