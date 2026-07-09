import type { Root } from "mdast";
import type { MdastNode } from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import { TYPE_NAMES } from "./generated/node-types.js";
import { materializeMdastFields } from "./generated/layout.js";

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

/** Lazy own children getter, shared per reader via `_nodeId`: O(1) in subtree size, no per-node closure. */
let lastReader: MdastReader | undefined;
let lastChildrenDescriptor: PropertyDescriptor | undefined;

function childrenDescriptor(reader: MdastReader): PropertyDescriptor {
  if (reader === lastReader) return lastChildrenDescriptor as PropertyDescriptor;
  const descriptor: PropertyDescriptor = {
    get(this: MdastNode) {
      const nodeId = (this as unknown as { _nodeId: number })._nodeId;
      const children = reader.getChildIds(nodeId).map((id) => materializeNode(reader, id));
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
  };
  lastReader = reader;
  lastChildrenDescriptor = descriptor;
  return descriptor;
}

/** Materialize a single MDAST node from a binary buffer; scalars eager, `children` lazy. */
export function materializeNode(reader: MdastReader, nodeId: number): MdastNode {
  const nodeType = reader.getNodeType(nodeId);
  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  const node = { type: typeName } as MdastNode;
  const position = reader.getPosition(nodeId);
  if (position !== undefined) {
    (node as { position: typeof position }).position = position;
  }

  // _nodeId: non-enumerable internal reference
  Object.defineProperty(node, "_nodeId", {
    value: nodeId,
    writable: false,
    configurable: true,
    enumerable: false,
  });

  // Type-specific lazy properties
  addTypeProperties(node, reader, nodeId, nodeType);

  // Plugin-set `data` survives the visitor walk via its own getter but
  // would be dropped when materialized from a serialized handle.
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
        console.warn(`materializeNode: malformed node_data for nodeId=${nodeId}`, err);
      }
    }
  }

  // children: lazy getter (only for non-leaf nodes)
  if (!LEAF_TYPES.has(nodeType)) {
    Object.defineProperty(node, "children", childrenDescriptor(reader));
  }

  return node;
}

/** Materialize the full tree from root (nodeId=0). */
export function materializeMdastTree(reader: MdastReader): Root {
  return materializeNode(reader, 0) as Root;
}
