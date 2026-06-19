import {
  HastReader,
  HAST_ROOT,
  HAST_ELEMENT,
  HAST_TEXT,
  HAST_COMMENT,
  HAST_DOCTYPE,
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
import { lazyProp, lazyGroup } from "../lazy-props.js";
import { restorePhantomSpaces } from "../phantom.js";

export type { HastNode };

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
 * Materialize a single HAST node from a binary buffer as a lazy JS object.
 */
export function materializeHastNode(reader: HastReader, nodeId: number): HastNode {
  const nodeType = reader.getNodeType(nodeId);
  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  const node = { type: typeName } as HastNode;
  // Position decodes to three objects; defer it — passthrough children (e.g.
  // replaceNode keeping `node.children`) never read it.
  if (reader.hasPosition(nodeId)) {
    Object.defineProperty(
      node,
      "position",
      lazyProp("position", () => reader.getPosition(nodeId)),
    );
  }

  // _nodeId: non-enumerable internal reference
  Object.defineProperty(node, "_nodeId", {
    value: nodeId,
    writable: false,
    configurable: true,
    enumerable: false,
  });

  switch (nodeType) {
    case HAST_ROOT:
      // children: lazy getter
      Object.defineProperty(node, "children", {
        get(this: HastNode) {
          const childIds = reader.getChildIds(nodeId);
          const children = childIds.map((id) => materializeHastNode(reader, id));
          Object.defineProperty(this, "children", {
            value: children,
            writable: true,
            configurable: true,
            enumerable: true,
          });
          return children;
        },
        configurable: true,
        enumerable: true,
      });
      break;

    case HAST_ELEMENT: {
      // tagName and properties: lazy, resolved together from one reader call
      lazyGroup(node, ["tagName", "properties"], () => {
        const { tagName, properties } = reader.getElementData(nodeId);
        return { tagName, properties: propsToRecord(properties) };
      });
      // children: lazy
      Object.defineProperty(node, "children", {
        get(this: HastNode) {
          const childIds = reader.getChildIds(nodeId);
          const children = childIds.map((id) => materializeHastNode(reader, id));
          Object.defineProperty(this, "children", {
            value: children,
            writable: true,
            configurable: true,
            enumerable: true,
          });
          return children;
        },
        configurable: true,
        enumerable: true,
      });
      break;
    }

    case HAST_TEXT:
    case HAST_COMMENT:
    case HAST_RAW:
      Object.defineProperties(node, {
        value: lazyProp("value", () => reader.getTextValue(nodeId)),
      });
      break;

    case HAST_DOCTYPE:
      // No extra properties
      break;

    case HAST_MDX_JSX_ELEMENT:
    case HAST_MDX_JSX_TEXT_ELEMENT:
      lazyGroup(node, ["name", "attributes"], () => reader.getMdxJsxElementData(nodeId));
      Object.defineProperty(node, "children", {
        get(this: HastNode) {
          const childIds = reader.getChildIds(nodeId);
          const children = childIds.map((id) => materializeHastNode(reader, id));
          Object.defineProperty(this, "children", {
            value: children,
            writable: true,
            configurable: true,
            enumerable: true,
          });
          return children;
        },
        configurable: true,
        enumerable: true,
      });
      break;

    case HAST_MDX_FLOW_EXPRESSION:
    case HAST_MDX_TEXT_EXPRESSION:
      Object.defineProperties(node, {
        value: lazyProp("value", () => restorePhantomSpaces(reader.getTextValue(nodeId))),
      });
      break;

    case HAST_MDX_ESM:
      Object.defineProperties(node, {
        value: lazyProp("value", () => reader.getTextValue(nodeId)),
      });
      break;
  }

  // Plugins can set `data` on any node type, so rehydrate generically
  // (see website/content/docs/divergences.md for the code-block case).
  const rawData = reader.getNodeData(nodeId);
  if (rawData !== null) {
    try {
      const parsed = JSON.parse(rawData) as Record<string, unknown>;
      if (parsed && typeof parsed === "object" && Object.keys(parsed).length > 0) {
        Object.defineProperty(node, "data", {
          value: parsed,
          writable: true,
          configurable: true,
          enumerable: true,
        });
      }
    } catch (err) {
      if (process.env.NODE_ENV !== "production") {
        console.warn(`materializeHastNode: malformed node_data for nodeId=${nodeId}`, err);
      }
    }
  }

  return node;
}

/**
 * Materialize the full HAST tree from root (nodeId=0).
 */
export function materializeHastTree(reader: HastReader): Root {
  return materializeHastNode(reader, 0) as Root;
}
