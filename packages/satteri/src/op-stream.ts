/**
 * Low-level op-stream writer shared by the MDAST/HAST declarative compilers.
 *
 * Emits the compact OPEN/CLOSE/field/REF/KEEP_CHILDREN/PROP stream that Rust
 * replays straight into the arena (`replay_opstream` in js_commands.rs; byte
 * values in generated/wire-constants.ts). The replay drives the SAME arena
 * encoders the JSON path uses, so a compiled tree is byte-identical to its
 * JSON form — it just skips the JSON + JsNode hop. Strings ride ByteWriter's
 * zero-alloc path (inline char codes when short ASCII, `encodeInto`
 * otherwise).
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

export class OpWriter {
  readonly #w = new ByteWriter(512);

  /** Reset for reuse; the grown buffer is retained so steady state is alloc-free. */
  reset(): void {
    this.#w.reset();
  }

  /** The op-stream written so far (valid until the next reset). */
  take(): Uint8Array {
    return this.#w.take();
  }

  open(type: number): void {
    const w = this.#w;
    w.ensure(2);
    w.u8(OP_OPEN);
    w.u8(type);
  }

  close(): void {
    const w = this.#w;
    w.ensure(1);
    w.u8(OP_CLOSE);
  }

  str(field: number, s: string): void {
    const w = this.#w;
    w.ensure(2);
    w.u8(OP_STR);
    w.u8(field);
    w.utf8WithU32Len(s);
  }

  u8(field: number, v: number): void {
    const w = this.#w;
    w.ensure(3);
    w.u8(OP_U8);
    w.u8(field);
    w.u8(v);
  }

  u32(field: number, v: number): void {
    const w = this.#w;
    w.ensure(6);
    w.u8(OP_U32);
    w.u8(field);
    w.u32(v);
  }

  bool(field: number, v: boolean): void {
    const w = this.#w;
    w.ensure(3);
    w.u8(OP_BOOL);
    w.u8(field);
    w.u8(v ? 1 : 0);
  }

  data(value: unknown): void {
    const w = this.#w;
    w.ensure(1);
    w.u8(OP_DATA);
    w.utf8WithU32Len(JSON.stringify(value));
  }

  prop(name: string, kind: number, value: string): void {
    const w = this.#w;
    w.ensure(1);
    w.u8(OP_PROP);
    w.utf8WithU32Len(name);
    w.ensure(1);
    w.u8(kind);
    w.utf8WithU32Len(value);
  }

  ref(id: number): void {
    const w = this.#w;
    w.ensure(5);
    w.u8(OP_REF);
    w.u32(id);
  }

  /** Table column-alignment codes (0=none, 1=left, 2=right, 3=center). */
  align(codes: readonly number[]): void {
    const w = this.#w;
    w.ensure(5 + codes.length);
    w.u8(OP_ALIGN);
    w.u32(codes.length);
    for (let i = 0; i < codes.length; i++) w.u8(codes[i]!);
  }

  keepChildren(): void {
    const w = this.#w;
    w.ensure(1);
    w.u8(OP_KEEP_CHILDREN);
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
