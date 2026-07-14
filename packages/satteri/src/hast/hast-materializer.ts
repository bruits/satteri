import {
  HastReader,
  HAST_ROOT,
  HAST_ELEMENT,
  HAST_TEXT,
  HAST_COMMENT,
  HAST_RAW,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
  HAST_MDX_FLOW_EXPRESSION,
  HAST_MDX_TEXT_EXPRESSION,
  HAST_MDX_ESM,
  type HastProperty,
} from "./hast-reader.js";
import type { Root } from "hast";
import type { HastNode } from "../types.js";
import { TYPE_NAMES } from "./generated/node-types.js";
import { restorePhantomSpaces } from "../phantom.js";
import { createMaterializer } from "../materializer-cache.js";

export type { HastNode };

/** Container node types (the ones that carry `children`); everything else is a leaf. */
export const HAST_CONTAINER_TYPES: ReadonlySet<number> = new Set([
  HAST_ROOT,
  HAST_ELEMENT,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
]);

function propsToRecord(
  props: HastProperty[],
): Record<string, string | number | boolean | string[]> {
  const result: Record<string, string | number | boolean | string[]> = {};
  for (const p of props) {
    result[p.name] = p.value;
  }
  return result;
}

/**
 * Materialize a single HAST node; scalars eager, `children` lazy, memoized per
 * `(reader, id)`; `frozen` (the plugin walk path) deep-freezes so plugins
 * cannot corrupt the shared cache.
 */
export const materializeHastNode = createMaterializer<HastReader, HastNode>({
  label: "materializeHastNode",
  typeNames: TYPE_NAMES,
  hasChildren: (nodeType) => HAST_CONTAINER_TYPES.has(nodeType),
  populate(node, reader, nodeId, nodeType) {
    switch (nodeType) {
      case HAST_ELEMENT: {
        const { tagName, properties } = reader.getElementData(nodeId);
        (node as { tagName: string }).tagName = tagName;
        (node as { properties: unknown }).properties = propsToRecord(properties);
        break;
      }

      case HAST_TEXT:
      case HAST_COMMENT:
      case HAST_RAW:
        (node as { value: string }).value = reader.getTextValue(nodeId);
        break;

      case HAST_MDX_JSX_ELEMENT:
      case HAST_MDX_JSX_TEXT_ELEMENT: {
        const { name, attributes } = reader.getMdxJsxElementData(nodeId);
        (node as { name: string | null }).name = name;
        (node as { attributes: unknown }).attributes = attributes;
        break;
      }

      case HAST_MDX_FLOW_EXPRESSION:
      case HAST_MDX_TEXT_EXPRESSION:
        (node as { value: string }).value = restorePhantomSpaces(reader.getTextValue(nodeId));
        break;

      case HAST_MDX_ESM:
        (node as { value: string }).value = reader.getTextValue(nodeId);
        break;

      // HAST_ROOT / HAST_DOCTYPE: no extra properties
      default:
        break;
    }
  },
});

/**
 * Materialize the full HAST tree from root (nodeId=0).
 */
export function materializeHastTree(reader: HastReader): Root {
  return materializeHastNode(reader, 0) as Root;
}
