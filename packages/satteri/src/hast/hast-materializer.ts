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
import type { HastNode } from "../types.js";
import { lazyProp, lazyGroup } from "../lazy-props.js";

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

  let typeName: string;
  switch (nodeType) {
    case HAST_ROOT:
      typeName = "root";
      break;
    case HAST_ELEMENT:
      typeName = "element";
      break;
    case HAST_TEXT:
      typeName = "text";
      break;
    case HAST_COMMENT:
      typeName = "comment";
      break;
    case HAST_DOCTYPE:
      typeName = "doctype";
      break;
    case HAST_RAW:
      typeName = "raw";
      break;
    case HAST_MDX_JSX_ELEMENT:
      typeName = "mdxJsxFlowElement";
      break;
    case HAST_MDX_JSX_TEXT_ELEMENT:
      typeName = "mdxJsxTextElement";
      break;
    case HAST_MDX_FLOW_EXPRESSION:
      typeName = "mdxFlowExpression";
      break;
    case HAST_MDX_TEXT_EXPRESSION:
      typeName = "mdxTextExpression";
      break;
    case HAST_MDX_ESM:
      typeName = "mdxjsEsm";
      break;
    default:
      typeName = `unknown(${nodeType})`;
      break;
  }

  const position = reader.getPosition(nodeId);
  const node = (position ? { type: typeName, position } : { type: typeName }) as HastNode;

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
export function materializeHastTree(reader: HastReader): HastNode {
  return materializeHastNode(reader, 0);
}
