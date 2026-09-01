/** Decode a HAST element property value from its wire `(kind, value)` — shared
 *  by the walk decoder and the snapshot reader so the kind dispatch lives once.
 *  The bool kinds carry no value string (callers pass `""`). */
import {
  PROP_BOOL_TRUE,
  PROP_BOOL_FALSE,
  PROP_SPACE_SEP,
  PROP_COMMA_SEP,
  PROP_COMMA_SEP_NUM,
  PROP_INT,
  PROP_TOKEN_LIST,
} from "../generated/wire-constants.js";

export type HastPropertyValue = string | number | boolean | (string | number)[];

/** Encode an array property value as `PROP_TOKEN_LIST`: whether it serializes
 *  comma- or space-separated depends on the element's schema, which is only
 *  known at render (a subtree may still be detached here). Every token is
 *  NUL-terminated, so an empty list stays distinct from a list holding one
 *  empty token.
 *
 *  A list *ending* in an empty string gets another appended, mirroring
 *  `comma-separated-tokens`, which pads so the value parses back to the same
 *  list. It happens here because only `""` pads: `null` joins to the same
 *  empty token but does not. Space-separated joining trims the padding away
 *  again, so it is harmless there. */
export function encodeTokenList(items: readonly unknown[]): string {
  if (items.length === 0) return "";
  const padded = items[items.length - 1] === "" ? [...items, ""] : items;
  return `${padded.map(tokenToWire).join("\0")}\0`;
}

/** U+0001 introduces an escape so a token carrying a NUL of its own does not
 *  read as two tokens: `\u00010` is a NUL, `\u00011` the escape itself. */
const ESCAPE = "\u0001";

/** `join` renders null and undefined as an empty token; keep that. */
function tokenToWire(item: unknown): string {
  const token = item === null || item === undefined ? "" : String(item);
  return token.includes("\0") || token.includes(ESCAPE)
    ? token.replaceAll(ESCAPE, `${ESCAPE}1`).replaceAll("\0", `${ESCAPE}0`)
    : token;
}

function decodeTokenList(value: string): string[] {
  if (value === "") return [];
  const tokens = (value.endsWith("\0") ? value.slice(0, -1) : value).split("\0");
  return tokens.map((token) =>
    token.includes(ESCAPE)
      ? token.replace(/\u0001([01])/g, (_, digit: string) => (digit === "0" ? "\0" : ESCAPE))
      : token,
  );
}

export function decodeElementProp(kind: number, value: string): HastPropertyValue {
  switch (kind) {
    case PROP_BOOL_TRUE:
      return true;
    case PROP_BOOL_FALSE:
      return false;
    case PROP_SPACE_SEP:
      return value.split(" ").filter((s) => s.length > 0);
    case PROP_COMMA_SEP: {
      // Interior empty items are kept; only a trailing empty is dropped.
      const items = value.split(",").map((s) => s.trim());
      if (items[items.length - 1] === "") items.pop();
      return items;
    }
    case PROP_COMMA_SEP_NUM: {
      const items = value.split(",").map((s) => s.trim());
      if (items[items.length - 1] === "") items.pop();
      return items.map((s) => (s !== "" && !Number.isNaN(Number(s)) ? Number(s) : s));
    }
    case PROP_TOKEN_LIST:
      return decodeTokenList(value);
    case PROP_INT:
      return Number(value);
    default:
      return value; // PROP_STRING
  }
}
