// Writing directly into CommandBuffer avoids an intermediate op-stream allocation and copy.

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
  OP_POSITION,
  MDX_ATTR_BOOLEAN_PROP,
  MDX_ATTR_LITERAL_PROP,
  MDX_ATTR_EXPRESSION_PROP,
  MDX_ATTR_SPREAD,
} from "./generated/wire-constants.js";
import {
  dataJson,
  describeValue,
  requireDirectiveAttributeValue,
  requireMdxAttributeValue,
  usageError,
} from "./usage-errors.js";

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
  PROP_INT,
  PROP_NULL,
  PROP_TOKEN_LIST,
} from "./generated/wire-constants.js";

export class OpWriter extends ByteWriter {
  constructor(initialSize = 512) {
    super(initialSize);
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
    const json = dataJson(value);
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

  /** Write a node's `position` so the node keeps it. Only lines and columns are
   *  sent: Rust derives the byte offsets the arena stores from them. */
  position(value: unknown): void {
    // Most re-encoded nodes are built by plugins and have no position: return before any work.
    if (value == null) return;
    const { start, end } = value as { start?: Point; end?: Point };
    if (start == null || end == null) return;
    const { line: startLine, column: startColumn } = start;
    const { line: endLine, column: endColumn } = end;
    if (
      !isLineOrColumn(startLine) ||
      !isLineOrColumn(startColumn) ||
      !isLineOrColumn(endLine) ||
      !isLineOrColumn(endColumn)
    ) {
      return;
    }
    this.ensure(17);
    this.buf[this.n++] = OP_POSITION;
    this.writeU32(startLine);
    this.writeU32(startColumn);
    this.writeU32(endLine);
    this.writeU32(endColumn);
  }
}

type Point = { line?: unknown; column?: unknown };

function isLineOrColumn(value: unknown): value is number {
  return Number.isInteger(value) && (value as number) >= 1 && (value as number) <= 4294967295;
}

export interface PropSink {
  prop(name: string, kind: number, value: string): void;
}

/** Keeps a list field's entries, which a field edit sends as JSON the way `data` goes. */
export class PropRows implements PropSink {
  readonly rows: [name: string, kind: number, value: string][] = [];

  prop(name: string, kind: number, value: string): void {
    this.rows.push([name, kind, value]);
  }
}

/** An MDX JSX element's `attributes` as `[name, kind, value]` entries. */
export function mdxAttributesJson(value: unknown): string {
  if (value === null || value === undefined) return "[]";
  if (!Array.isArray(value)) {
    throw usageError(
      `an MDX JSX element's "attributes" is an array of attributes, not ${describeValue(value)}`,
      TypeError,
    );
  }
  const rows = new PropRows();
  for (const attribute of value) emitMdxAttr(rows, attribute);
  return JSON.stringify(rows.rows);
}

/** A directive's `attributes` object as `[key, value]` pairs. */
export function directiveAttributesJson(value: unknown): string {
  if (value === null || value === undefined) return "[]";
  if (typeof value !== "object" || Array.isArray(value)) {
    throw usageError(
      `a directive's "attributes" is an object of strings, not ${describeValue(value)}`,
      TypeError,
    );
  }
  const pairs: [string, string][] = [];
  for (const key in value as Record<string, unknown>) {
    const v = (value as Record<string, unknown>)[key];
    requireDirectiveAttributeValue(key, v, true);
    if (typeof v === "string") pairs.push([key, v]);
  }
  return JSON.stringify(pairs);
}

export function emitMdxAttr(w: PropSink, attribute: unknown): void {
  if (attribute === null || typeof attribute !== "object") {
    throw usageError(
      `an MDX attribute is { type: "mdxJsxAttribute", name, value }, not ${describeValue(attribute)}`,
      TypeError,
    );
  }
  const a = attribute as Record<string, unknown>;
  if (a.type === "mdxJsxExpressionAttribute") {
    if (typeof a.value !== "string") {
      throw usageError(
        `a spread attribute's value is its expression source, a string, not ${describeValue(a.value)}`,
        TypeError,
      );
    }
    w.prop("", MDX_ATTR_SPREAD, a.value);
    return;
  }
  const name = a.name;
  if (typeof name !== "string" || name === "") {
    throw usageError(
      `an MDX attribute needs a name, not ${describeValue(name)}; a spread is ` +
        '{ type: "mdxJsxExpressionAttribute", value: "...props" }',
      TypeError,
    );
  }
  const val = a.value;
  requireMdxAttributeValue(name, val);
  if (typeof val === "string") w.prop(name, MDX_ATTR_LITERAL_PROP, val);
  else if (val === null || val === undefined || val === true)
    w.prop(name, MDX_ATTR_BOOLEAN_PROP, "");
  else w.prop(name, MDX_ATTR_EXPRESSION_PROP, (val as { value: string }).value);
}
