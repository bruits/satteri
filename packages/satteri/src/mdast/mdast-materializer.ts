import type { Root } from "mdast";
import type { MdastNode } from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import { TYPE_NAMES } from "./generated/node-types.js";
import { materializeMdastFields } from "./generated/layout.js";
import { createMaterializer } from "../materializer-cache.js";

// Leaf node types that do NOT have children.
// Type 9 = `definition`; type 18 = `imageReference` — leaves per mdast spec
// (imageReference carries `alt` as a string, not children).
export const LEAF_TYPES: ReadonlySet<number> = new Set([
  9, 10, 13, 7, 8, 14, 3, 16, 18, 20, 25, 26, 27, 28, 102, 103, 104,
]);

/**
 * Add type-specific properties to a node object as eager plain stores.
 */
function addTypeProperties(
  node: MdastNode,
  reader: MdastReader,
  nodeId: number,
  nodeType: number,
): void {
  // Fixed-field types materialize from the generated layout table; the rest
  // (variable-length / cross-field) stay in the hand-written switch.
  if (materializeMdastFields(reader, node, nodeId, nodeType)) return;

  switch (nodeType) {
    case 5: {
      // list
      const d = reader.getListData(nodeId);
      const n = node as { ordered: boolean; start: number | null; spread: boolean };
      n.ordered = d.ordered;
      n.start = d.ordered ? d.start : null;
      n.spread = d.spread;
      break;
    }

    case 6: {
      // listItem
      const d = reader.getListItemData(nodeId);
      const n = node as { spread: boolean; checked: boolean | null };
      n.spread = d.spread;
      n.checked = d.checked;
      break;
    }

    case 37: {
      // descriptionDetails
      const d = reader.getDescriptionDetailsData(nodeId);
      (node as { spread: boolean }).spread = d.spread;
      break;
    }

    case 21: // table
      (node as { align: unknown }).align = reader.getTableAlign(nodeId);
      break;

    case 30: // containerDirective
    case 31: // leafDirective
    case 32: {
      // textDirective
      const d = reader.getDirectiveData(nodeId);
      const n = node as { name: string; attributes: unknown };
      n.name = d.name;
      n.attributes = d.attributes;
      break;
    }

    case 100: // mdxJsxFlowElement
    case 101: {
      // mdxJsxTextElement
      const d = reader.getMdxJsxElementData(nodeId);
      const n = node as { name: string | null; attributes: unknown };
      n.name = d.name;
      n.attributes = d.attributes;
      break;
    }

    // Nodes with no type-specific props:
    // root(0), paragraph(1), thematicBreak(3), blockquote(4),
    // emphasis(11), strong(12), break(14), tableRow(22), tableCell(23), delete(24)
    default:
      break;
  }
}

/**
 * Materialize a single MDAST node; scalars eager, `children` lazy, memoized
 * per `(reader, id)`; `frozen` (the plugin walk path) deep-freezes so plugins
 * cannot corrupt the shared cache.
 */
export const materializeNode = createMaterializer<MdastReader, MdastNode>({
  label: "materializeNode",
  typeNames: TYPE_NAMES,
  hasChildren: (nodeType) => !LEAF_TYPES.has(nodeType),
  populate: addTypeProperties,
});

/** Materialize the full tree from root (nodeId=0). */
export function materializeMdastTree(reader: MdastReader): Root {
  return materializeNode(reader, 0) as Root;
}
