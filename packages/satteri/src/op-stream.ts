/**
 * Low-level op-stream writer shared by the MDAST/HAST declarative compilers.
 *
 * Emits the compact OPEN/CLOSE/field/REF/KEEP_CHILDREN/PROP stream that Rust
 * replays straight into the arena (`replay_opstream` in js_commands.rs; byte
 * values in generated/wire-constants.ts) — the only structural encoding for
 * plugin-built content. Strings ride ByteWriter's zero-alloc path (inline
 * char codes when short ASCII, `encodeInto` otherwise).
 */

import { ByteWriter } from "./byte-writer.js";
import {
  OP_OPEN,
  OP_CLOSE,
  OP_REF,
  OP_KEEP_CHILDREN,
  OP_STR,
  OP_U8,
  OP_U32,
  OP_BOOL,
  OP_DATA,
  OP_PROP,
  OP_ALIGN,
  MDX_ATTR_BOOLEAN_PROP,
  MDX_ATTR_LITERAL_PROP,
  MDX_ATTR_EXPRESSION_PROP,
  MDX_ATTR_SPREAD,
} from "./generated/wire-constants.js";

// Re-exported so visitors/readers keep importing wire constants from here.
export {
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
  OF_TAGNAME,
  OF_EXPLICIT,
  PROP_STRING,
  PROP_BOOL_TRUE,
  PROP_BOOL_FALSE,
  PROP_SPACE_SEP,
  PROP_COMMA_SEP,
  PROP_INT,
  PROP_NULL,
  MDX_ATTR_BOOLEAN_PROP,
  MDX_ATTR_LITERAL_PROP,
  MDX_ATTR_EXPRESSION_PROP,
  MDX_ATTR_SPREAD,
} from "./generated/wire-constants.js";

export class OpWriter extends ByteWriter {
  #encoding = false;

  constructor() {
    super(512);
  }

  /**
   * Start a compile on this (shared, module-level) writer. Encoding evaluates
   * plugin-supplied getters and `toJSON`; if one of those calls a context
   * mutation, the nested compile would reset the stream under the outer one
   * and corrupt it silently — throw instead. Pair with `end` in a `finally`.
   */
  begin(): void {
    if (this.#encoding) {
      throw new Error(
        "reentrant op-stream compile: a node getter or toJSON invoked a context mutation while its node was being encoded",
      );
    }
    this.#encoding = true;
    this.reset();
  }

  end(): void {
    this.#encoding = false;
  }

  open(type: number): void {
    this.ensure(2);
    this.buf[this.n++] = OP_OPEN;
    this.buf[this.n++] = type;
  }

  close(): void {
    this.ensure(1);
    this.buf[this.n++] = OP_CLOSE;
  }

  str(field: number, s: string): void {
    this.ensure(2);
    this.buf[this.n++] = OP_STR;
    this.buf[this.n++] = field;
    this.utf8WithU32Len(s);
  }

  u8(field: number, v: number): void {
    this.ensure(3);
    this.buf[this.n++] = OP_U8;
    this.buf[this.n++] = field;
    this.buf[this.n++] = v & 255;
  }

  u32(field: number, v: number): void {
    this.ensure(6);
    this.buf[this.n++] = OP_U32;
    this.buf[this.n++] = field;
    this.writeU32(v);
  }

  bool(field: number, v: boolean): void {
    this.ensure(3);
    this.buf[this.n++] = OP_BOOL;
    this.buf[this.n++] = field;
    this.buf[this.n++] = v ? 1 : 0;
  }

  data(value: unknown): void {
    const json = JSON.stringify(value);
    // `JSON.stringify` yields undefined for a function or a `toJSON`
    // returning undefined; a clear error beats a TypeError deep in the
    // string encoder.
    if (typeof json !== "string") {
      throw new Error("node `data` is not JSON-serializable");
    }
    this.ensure(1);
    this.buf[this.n++] = OP_DATA;
    this.utf8WithU32Len(json);
  }

  prop(name: string, kind: number, value: string): void {
    this.ensure(1);
    this.buf[this.n++] = OP_PROP;
    this.utf8WithU32Len(name);
    this.ensure(1);
    this.buf[this.n++] = kind;
    this.utf8WithU32Len(value);
  }

  ref(id: number): void {
    this.ensure(5);
    this.buf[this.n++] = OP_REF;
    this.writeU32(id);
  }

  /** Table column-alignment codes (0=none, 1=left, 2=right, 3=center). */
  align(codes: readonly number[]): void {
    this.ensure(5 + codes.length);
    this.buf[this.n++] = OP_ALIGN;
    this.writeU32(codes.length);
    for (let i = 0; i < codes.length; i++) this.buf[this.n++] = codes[i]! & 255;
  }

  keepChildren(): void {
    this.ensure(1);
    this.buf[this.n++] = OP_KEEP_CHILDREN;
  }
}

/** Emit one MDX JSX attribute as an OP_PROP, mirroring `encode_js_jsx_attrs`:
 *  null→boolean, string→literal, `{ value }` object→expression, else→boolean. */
export function emitMdxAttr(w: OpWriter, a: Record<string, unknown>): void {
  if (a.type === "mdxJsxExpressionAttribute") {
    w.prop("", MDX_ATTR_SPREAD, typeof a.value === "string" ? a.value : "");
    return;
  }
  const name = typeof a.name === "string" ? a.name : "";
  const val = a.value;
  if (typeof val === "string") {
    w.prop(name, MDX_ATTR_LITERAL_PROP, val);
  } else if (val !== null && typeof val === "object" && !Array.isArray(val)) {
    const expr = (val as Record<string, unknown>).value;
    w.prop(name, MDX_ATTR_EXPRESSION_PROP, typeof expr === "string" ? expr : "");
  } else {
    w.prop(name, MDX_ATTR_BOOLEAN_PROP, "");
  }
}
