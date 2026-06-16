/** Decode a HAST element property value from its wire `(kind, value)` — shared
 *  by the walk decoder and the snapshot reader so the kind dispatch lives once.
 *  The bool kinds carry no value string (callers pass `""`). */
import {
  PROP_BOOL_TRUE,
  PROP_BOOL_FALSE,
  PROP_SPACE_SEP,
  PROP_COMMA_SEP,
  PROP_INT,
} from "../generated/wire-constants.js";

export type HastPropertyValue = string | number | boolean | string[];

export function decodeElementProp(kind: number, value: string): HastPropertyValue {
  switch (kind) {
    case PROP_BOOL_TRUE:
      return true;
    case PROP_BOOL_FALSE:
      return false;
    case PROP_SPACE_SEP:
      return value.split(" ").filter((s) => s.length > 0);
    case PROP_COMMA_SEP:
      return value
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
    case PROP_INT:
      return Number(value);
    default:
      return value; // PROP_STRING
  }
}
