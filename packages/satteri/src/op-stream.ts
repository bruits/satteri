/**
 * Low-level op-stream writer shared by the MDAST/HAST declarative compilers.
 *
 * Emits the compact OPEN/CLOSE/field/REF/KEEP_CHILDREN/PROP stream that Rust
 * replays straight into the arena (see js_commands.rs `OP_*` / `OF_*`). The
 * replay drives the SAME arena encoders the JSON path uses, so a compiled tree
 * is byte-identical to its JSON form — it just skips the JSON + JsNode hop.
 * Strings take an ASCII fast path (char codes) to avoid a per-string encoder.
 */

// Op codes (must match js_commands.rs OP_*).
const OP_OPEN = 0x01;
const OP_CLOSE = 0x02;
const OP_REF = 0x03;
const OP_KEEP_CHILDREN = 0x04;
const OP_STR = 0x05;
const OP_U8 = 0x06;
const OP_U32 = 0x07;
const OP_BOOL = 0x08;
const OP_DATA = 0x09;
const OP_PROP = 0x0a;
const OP_ALIGN = 0x0b;

// Field ids (must match js_commands.rs OF_*).
export const OF_VALUE = 0;
export const OF_URL = 1;
export const OF_TITLE = 2;
export const OF_ALT = 3;
export const OF_LANG = 4;
export const OF_META = 5;
export const OF_IDENTIFIER = 6;
export const OF_LABEL = 7;
export const OF_NAME = 8;
export const OF_REFERENCE_TYPE = 9;
export const OF_DEPTH = 10;
export const OF_CHECKED = 11;
export const OF_START = 12;
export const OF_ORDERED = 13;
export const OF_SPREAD = 14;
export const OF_TAGNAME = 15;
export const OF_EXPLICIT = 16;

// Property value kinds (must match shared::PROP_*).
export const PROP_STRING = 0;
export const PROP_BOOL_TRUE = 1;
export const PROP_BOOL_FALSE = 2;
export const PROP_SPACE_SEP = 3;
export const PROP_INT = 5;

// MDX JSX attribute kinds (must match shared::MDX_ATTR_*).
const MDX_ATTR_BOOLEAN = 0;
const MDX_ATTR_LITERAL = 1;
const MDX_ATTR_EXPRESSION = 2;
const MDX_ATTR_SPREAD = 3;

const encoder = new TextEncoder();

export class OpWriter {
  #buf = new Uint8Array(512);
  #n = 0;

  /** The op-stream written so far (valid until the next reset). */
  take(): Uint8Array {
    return this.#buf.subarray(0, this.#n);
  }

  #ensure(k: number): void {
    if (this.#n + k <= this.#buf.length) return;
    let size = this.#buf.length * 2;
    while (this.#n + k > size) size *= 2;
    const grown = new Uint8Array(size);
    grown.set(this.#buf);
    this.#buf = grown;
  }

  #u32at(v: number): void {
    this.#buf[this.#n++] = v & 255;
    this.#buf[this.#n++] = (v >> 8) & 255;
    this.#buf[this.#n++] = (v >> 16) & 255;
    this.#buf[this.#n++] = (v >>> 24) & 255;
  }

  #string(s: string): void {
    const len = s.length;
    this.#ensure(4 + len);
    let ascii = true;
    for (let i = 0; i < len; i++) {
      if (s.charCodeAt(i) > 127) {
        ascii = false;
        break;
      }
    }
    if (ascii) {
      this.#u32at(len);
      const buf = this.#buf;
      const n = this.#n;
      for (let i = 0; i < len; i++) buf[n + i] = s.charCodeAt(i);
      this.#n = n + len;
    } else {
      const bytes = encoder.encode(s);
      this.#ensure(4 + bytes.length);
      this.#u32at(bytes.length);
      this.#buf.set(bytes, this.#n);
      this.#n += bytes.length;
    }
  }

  open(type: number): void {
    this.#ensure(2);
    this.#buf[this.#n++] = OP_OPEN;
    this.#buf[this.#n++] = type;
  }

  close(): void {
    this.#ensure(1);
    this.#buf[this.#n++] = OP_CLOSE;
  }

  str(field: number, s: string): void {
    this.#ensure(2);
    this.#buf[this.#n++] = OP_STR;
    this.#buf[this.#n++] = field;
    this.#string(s);
  }

  u8(field: number, v: number): void {
    this.#ensure(3);
    this.#buf[this.#n++] = OP_U8;
    this.#buf[this.#n++] = field;
    this.#buf[this.#n++] = v & 255;
  }

  u32(field: number, v: number): void {
    this.#ensure(6);
    this.#buf[this.#n++] = OP_U32;
    this.#buf[this.#n++] = field;
    this.#u32at(v);
  }

  bool(field: number, v: boolean): void {
    this.#ensure(3);
    this.#buf[this.#n++] = OP_BOOL;
    this.#buf[this.#n++] = field;
    this.#buf[this.#n++] = v ? 1 : 0;
  }

  data(value: unknown): void {
    const bytes = encoder.encode(JSON.stringify(value));
    this.#ensure(5 + bytes.length);
    this.#buf[this.#n++] = OP_DATA;
    this.#u32at(bytes.length);
    this.#buf.set(bytes, this.#n);
    this.#n += bytes.length;
  }

  prop(name: string, kind: number, value: string): void {
    this.#ensure(1);
    this.#buf[this.#n++] = OP_PROP;
    this.#string(name);
    this.#ensure(1);
    this.#buf[this.#n++] = kind;
    this.#string(value);
  }

  ref(id: number): void {
    this.#ensure(5);
    this.#buf[this.#n++] = OP_REF;
    this.#u32at(id);
  }

  /** Table column-alignment codes (0=none, 1=left, 2=right, 3=center). */
  align(codes: readonly number[]): void {
    this.#ensure(5 + codes.length);
    this.#buf[this.#n++] = OP_ALIGN;
    this.#u32at(codes.length);
    for (let i = 0; i < codes.length; i++) this.#buf[this.#n++] = codes[i]! & 255;
  }

  keepChildren(): void {
    this.#ensure(1);
    this.#buf[this.#n++] = OP_KEEP_CHILDREN;
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
    w.prop(name, MDX_ATTR_LITERAL, val);
  } else if (val !== null && typeof val === "object" && !Array.isArray(val)) {
    const expr = (val as Record<string, unknown>).value;
    w.prop(name, MDX_ATTR_EXPRESSION, typeof expr === "string" ? expr : "");
  } else {
    w.prop(name, MDX_ATTR_BOOLEAN, "");
  }
}
