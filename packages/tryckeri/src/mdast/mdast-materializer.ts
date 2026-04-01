import type { MdastNode } from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import type { DataMap } from "../data-map.js";
import { lazyProp, lazyGroup } from "../lazy-props.js";

export const TYPE_NAMES: Record<number, string> = {
  0: "root",
  1: "paragraph",
  2: "heading",
  3: "thematicBreak",
  4: "blockquote",
  5: "list",
  6: "listItem",
  7: "html",
  8: "code",
  9: "definition",
  10: "text",
  11: "emphasis",
  12: "strong",
  13: "inlineCode",
  14: "break",
  15: "link",
  16: "image",
  17: "linkReference",
  18: "imageReference",
  19: "footnoteDefinition",
  20: "footnoteReference",
  21: "table",
  22: "tableRow",
  23: "tableCell",
  24: "delete",
  25: "yaml",
  26: "toml",
  27: "math",
  28: "inlineMath",
  100: "mdxJsxFlowElement",
  101: "mdxJsxTextElement",
  102: "mdxFlowExpression",
  103: "mdxTextExpression",
  104: "mdxjsEsm",
};

// Leaf node types that do NOT have children
const LEAF_TYPES = new Set([10, 13, 7, 8, 14, 3, 20, 25, 26, 27, 28, 102, 103, 104]);

/**
 * Add type-specific lazy properties to a node object.
 */
function addTypeProperties(
  node: MdastNode,
  reader: MdastReader,
  nodeId: number,
  nodeType: number,
): void {
  switch (nodeType) {
    case 2: // heading
      Object.defineProperties(node, {
        depth: lazyProp("depth", () => reader.getHeadingDepth(nodeId)),
      });
      break;

    case 10: // text
    case 13: // inlineCode
    case 7: // html
    case 25: // yaml
    case 26: // toml
    case 28: // inlineMath
      Object.defineProperties(node, {
        value: lazyProp("value", () => reader.getTextValue(nodeId)),
      });
      break;

    case 8: // code
      lazyGroup(node, ["lang", "meta", "value"], () => reader.getCodeData(nodeId));
      break;

    case 27: // math
      lazyGroup(node, ["meta", "value"], () => reader.getMathData(nodeId));
      break;

    case 15: // link
      lazyGroup(node, ["url", "title"], () => reader.getLinkData(nodeId));
      break;

    case 9: // definition
      lazyGroup(node, ["url", "title", "identifier", "label"], () =>
        reader.getDefinitionData(nodeId),
      );
      break;

    case 16: // image
      lazyGroup(node, ["url", "alt", "title"], () => reader.getImageData(nodeId));
      break;

    case 5: {
      // list
      const resolveList = () => {
        const d = reader.getListData(nodeId);
        return { ordered: d.ordered, start: d.ordered ? d.start : null, spread: d.spread };
      };
      lazyGroup(node, ["ordered", "start", "spread"], resolveList);
      break;
    }

    case 6: // listItem
      lazyGroup(node, ["checked", "spread"], () => reader.getListItemData(nodeId));
      break;

    case 17: // linkReference
    case 18: // imageReference
    case 20: // footnoteReference
      lazyGroup(node, ["identifier", "label", "referenceType"], () =>
        reader.getReferenceData(nodeId),
      );
      break;

    case 19: // footnoteDefinition
      lazyGroup(node, ["identifier", "label"], () => reader.getFootnoteDefinitionData(nodeId));
      break;

    case 21: // table
      Object.defineProperties(node, {
        align: lazyProp("align", () => reader.getTableAlign(nodeId)),
      });
      break;

    case 100: // mdxJsxFlowElement
    case 101: // mdxJsxTextElement
      lazyGroup(node, ["name", "attributes"], () => reader.getMdxJsxElementData(nodeId));
      break;

    case 102: // mdxFlowExpression
    case 103: // mdxTextExpression
    case 104: // mdxjsEsm
      Object.defineProperties(node, {
        value: lazyProp("value", () => reader.getExpressionValue(nodeId)),
      });
      break;

    // Nodes with no type-specific props:
    // root(0), paragraph(1), thematicBreak(3), blockquote(4),
    // emphasis(11), strong(12), break(14), tableRow(22), tableCell(23), delete(24)
    default:
      break;
  }
}

/**
 * Materialize a single MDAST node from a binary buffer as a lazy JS object.
 */
export function materializeNode(reader: MdastReader, nodeId: number, dataMap: DataMap): MdastNode {
  const rawNode = reader.getNode(nodeId);
  const nodeType = rawNode.type;
  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  const node = {
    type: typeName,
    position: rawNode.position,
  } as MdastNode;

  // _nodeId: non-enumerable internal reference
  Object.defineProperty(node, "_nodeId", {
    value: nodeId,
    writable: false,
    configurable: true,
    enumerable: false,
  });

  // data: getter/setter backed by the DataMap
  Object.defineProperty(node, "data", {
    get() {
      return dataMap.get(nodeId);
    },
    set(value: Record<string, unknown>) {
      dataMap.set(nodeId, value);
    },
    configurable: true,
    enumerable: true,
  });

  // Type-specific lazy properties
  addTypeProperties(node, reader, nodeId, nodeType);

  // children: lazy getter (only for non-leaf nodes)
  if (!LEAF_TYPES.has(nodeType)) {
    Object.defineProperty(node, "children", {
      get(this: MdastNode) {
        const childIds = reader.getChildIds(nodeId);
        const children = childIds.map((id) => materializeNode(reader, id, dataMap));
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
  }

  return node;
}

/** Materialize the full tree from root (nodeId=0). */
export function materializeTree(reader: MdastReader, dataMap: DataMap): MdastNode {
  return materializeNode(reader, 0, dataMap);
}
