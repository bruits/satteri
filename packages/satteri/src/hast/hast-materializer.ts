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

/** Node memo + shared lazy `children` descriptor; the memo keeps one object per `(reader, id)` so identity-based plugin dedup works across access paths. */
interface ReaderCache {
  nodes: Map<number, HastNode>;
  children: PropertyDescriptor;
}

const READER_CACHES = new WeakMap<HastReader, ReaderCache>();

function readerCache(reader: HastReader): ReaderCache {
  let cache = READER_CACHES.get(reader);
  if (cache === undefined) {
    const children: PropertyDescriptor = {
      get(this: HastNode) {
        const nodeId = (this as unknown as { _nodeId: number })._nodeId;
        const value = reader.getChildIds(nodeId).map((id) => materializeHastNode(reader, id));
        Object.defineProperty(this, "children", {
          value,
          writable: true,
          configurable: true,
          enumerable: true,
        });
        return value;
      },
      configurable: true,
      enumerable: true,
    };
    cache = { nodes: new Map(), children };
    READER_CACHES.set(reader, cache);
  }
  return cache;
}

/** Materialize a single HAST node; scalars eager, `children` lazy, memoized per `(reader, id)`. */
export function materializeHastNode(reader: HastReader, nodeId: number): HastNode {
  const cache = readerCache(reader);
  let node = cache.nodes.get(nodeId);
  if (node === undefined) {
    node = buildHastNode(reader, cache, nodeId);
    cache.nodes.set(nodeId, node);
  }
  return node;
}

function buildHastNode(reader: HastReader, cache: ReaderCache, nodeId: number): HastNode {
  const nodeType = reader.getNodeType(nodeId);
  const typeName = TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`;

  const node = { type: typeName } as HastNode;
  const position = reader.getPosition(nodeId);
  if (position !== undefined) {
    node.position = position;
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
      Object.defineProperty(node, "children", cache.children);
      break;

    case HAST_ELEMENT: {
      const { tagName, properties } = reader.getElementData(nodeId);
      (node as { tagName: string }).tagName = tagName;
      (node as { properties: unknown }).properties = propsToRecord(properties);
      Object.defineProperty(node, "children", cache.children);
      break;
    }

    case HAST_TEXT:
    case HAST_COMMENT:
    case HAST_RAW:
      (node as { value: string }).value = reader.getTextValue(nodeId);
      break;

    case HAST_DOCTYPE:
      // No extra properties
      break;

    case HAST_MDX_JSX_ELEMENT:
    case HAST_MDX_JSX_TEXT_ELEMENT: {
      const { name, attributes } = reader.getMdxJsxElementData(nodeId);
      (node as { name: string | null }).name = name;
      (node as { attributes: unknown }).attributes = attributes;
      Object.defineProperty(node, "children", cache.children);
      break;
    }

    case HAST_MDX_FLOW_EXPRESSION:
    case HAST_MDX_TEXT_EXPRESSION:
      (node as { value: string }).value = restorePhantomSpaces(reader.getTextValue(nodeId));
      break;

    case HAST_MDX_ESM:
      (node as { value: string }).value = reader.getTextValue(nodeId);
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
