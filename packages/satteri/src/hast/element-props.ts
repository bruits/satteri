// Boolean property kinds carry no value string in the wire protocol.
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
 *  known at render (a subtree may still be detached here). A leading `1`/`0`
 *  records whether comma joining needs trailing padding. Each NUL-terminated
 *  token starts with `n` (number) or `s` (string), preserving numeric items
 *  when a later plugin reads the property.
 *
 *  A list *ending* in an empty string gets another appended, mirroring
 *  `comma-separated-tokens`, which pads so the value parses back to the same
 *  list. Only `""` pads: `null` joins to the same empty token but does not.
 *  Keep padding separate from the items so materializing and re-encoding
 *  a property never adds phantom tokens. */
export function encodeTokenList(items: readonly unknown[]): string {
  if (items.length === 0) return "";
  let tokens = items[items.length - 1] === "" ? "1" : "0";
  for (const item of items) tokens += `${tokenToWire(item)}\0`;
  return tokens;
}

/** U+0001 introduces an escape so a token carrying a NUL of its own does not
 *  read as two tokens: `\u00010` is a NUL, `\u00011` the escape itself. */
const ESCAPE = "\u0001";

/** `join` renders null and undefined as an empty token; keep that. */
function tokenToWire(item: unknown): string {
  const token = item === null || item === undefined ? "" : String(item);
  const escaped =
    token.includes("\0") || token.includes(ESCAPE)
      ? token.replaceAll(ESCAPE, `${ESCAPE}1`).replaceAll("\0", `${ESCAPE}0`)
      : token;
  return `${typeof item === "number" ? "n" : "s"}${escaped}`;
}

function decodeTokenList(value: string): (string | number)[] {
  if (value === "") return [];
  const body = value.slice(1);
  const tokens = (body.endsWith("\0") ? body.slice(0, -1) : body).split("\0");
  return tokens.map((entry) => {
    const token = entry.slice(1);
    if (entry[0] === "n") return Number(token);
    return token.includes(ESCAPE)
      ? token.replace(/\u0001([01])/g, (_, digit: string) => (digit === "0" ? "\0" : ESCAPE))
      : token;
  });
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
      return value;
  }
}
