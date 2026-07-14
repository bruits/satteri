/**
 * Shared materializer machinery for the HAST and MDAST flavors: per-reader
 * node memo, lazy `children` descriptors, and the frozen-mode (plugin walk
 * path) freeze rules.
 */

import type { Position } from "unist";
import { deepFreeze } from "./freeze.js";

/** The reader surface the shared machinery needs; both `HastReader` and `MdastReader` satisfy it. */
export interface MaterializerReader {
  getNodeType(nodeId: number): number;
  getChildIds(nodeId: number): number[];
  getPosition(nodeId: number): Position | undefined;
  getNodeData(nodeId: number): string | null;
}

/** Node memo + shared lazy `children` descriptor; the memo keeps one object per `(reader, id)` so identity-based plugin dedup works across access paths. */
interface ReaderCache<TNode extends object> {
  nodes: Map<number, TNode>;
  /** Frozen-mode memo of children arrays, keyed by node id. */
  childLists: Map<number, readonly TNode[]>;
  children: PropertyDescriptor;
  frozen: boolean;
}

export interface MaterializerSpec<TReader extends MaterializerReader, TNode extends object> {
  /** Function name used in error/warning messages (e.g. "materializeHastNode"). */
  label: string;
  /** Node-type tag -> canonical AST name (the generated `TYPE_NAMES`). */
  typeNames: Readonly<Record<number, string>>;
  /** Whether nodes of this type carry `children`. */
  hasChildren(nodeType: number): boolean;
  /**
   * Install the type-specific eager fields on `node`. Must not install
   * `children`, `position`, `data`, or `_nodeId`, and must not freeze —
   * the shared machinery owns all of those.
   */
  populate(node: TNode, reader: TReader, nodeId: number, nodeType: number): void;
}

/**
 * Build a memoizing materializer: scalars eager, `children` lazy, memoized per
 * `(reader, id)`; `frozen` (the plugin walk path) deep-freezes every node at
 * construction so plugins cannot corrupt the shared cache.
 */
export function createMaterializer<TReader extends MaterializerReader, TNode extends object>(
  spec: MaterializerSpec<TReader, TNode>,
): (reader: TReader, nodeId: number, frozen?: boolean) => TNode {
  const readerCaches = new WeakMap<TReader, ReaderCache<TNode>>();

  function materialize(reader: TReader, nodeId: number, frozen = false): TNode {
    const cache = readerCache(reader, frozen);
    let node = cache.nodes.get(nodeId);
    if (node === undefined) {
      node = buildNode(reader, cache, nodeId);
      cache.nodes.set(nodeId, node);
    }
    return node;
  }

  /** Frozen-mode `children`: memoized in `cache.childLists` because the node
   *  is frozen, so the accessor cannot self-replace with a data property. */
  function frozenChildrenDescriptor(
    reader: TReader,
    cache: ReaderCache<TNode>,
  ): PropertyDescriptor {
    return {
      get(this: TNode): readonly TNode[] {
        const nodeId = (this as unknown as { _nodeId: number })._nodeId;
        let value = cache.childLists.get(nodeId);
        if (value === undefined) {
          value = Object.freeze(
            reader.getChildIds(nodeId).map((id) => materialize(reader, id, true)),
          );
          cache.childLists.set(nodeId, value);
        }
        return value;
      },
      configurable: true,
      enumerable: true,
    };
  }

  /** Mutable-mode `children`: self-replacing with a plain writable array on first read. */
  function mutableChildrenDescriptor(reader: TReader): PropertyDescriptor {
    return {
      get(this: TNode): TNode[] {
        const nodeId = (this as unknown as { _nodeId: number })._nodeId;
        const value = reader.getChildIds(nodeId).map((id) => materialize(reader, id));
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
  }

  function readerCache(reader: TReader, frozen: boolean): ReaderCache<TNode> {
    let cache = readerCaches.get(reader);
    if (cache === undefined) {
      cache = {
        nodes: new Map(),
        childLists: new Map(),
        children: undefined as unknown as PropertyDescriptor,
        frozen,
      };
      cache.children = frozen
        ? frozenChildrenDescriptor(reader, cache)
        : mutableChildrenDescriptor(reader);
      readerCaches.set(reader, cache);
    }
    if (cache.frozen !== frozen) {
      throw new Error(`${spec.label}: a reader cannot mix frozen and mutable materialization`);
    }
    return cache;
  }

  function buildNode(reader: TReader, cache: ReaderCache<TNode>, nodeId: number): TNode {
    const nodeType = reader.getNodeType(nodeId);
    const typeName = spec.typeNames[nodeType] ?? `unknown(${nodeType})`;

    const node = { type: typeName } as unknown as TNode;
    const position = reader.getPosition(nodeId);
    if (position !== undefined) {
      (node as { position?: Position }).position = position;
    }

    // _nodeId: non-enumerable internal reference
    Object.defineProperty(node, "_nodeId", {
      value: nodeId,
      writable: false,
      configurable: true,
      enumerable: false,
    });

    spec.populate(node, reader, nodeId, nodeType);

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
          console.warn(`${spec.label}: malformed node_data for nodeId=${nodeId}`, err);
        }
      }
    }

    if (spec.hasChildren(nodeType)) {
      Object.defineProperty(node, "children", cache.children);
    }

    if (cache.frozen) {
      // Deep-freeze the eager own values but not the lazy `children` accessor;
      // freeze eagerly even for containers so nothing is writable while cached.
      const descriptors = Object.getOwnPropertyDescriptors(node);
      for (const key of Object.keys(descriptors)) {
        const desc = descriptors[key];
        if (desc !== undefined && "value" in desc) {
          deepFreeze(desc.value);
        }
      }
      Object.freeze(node);
    }

    return node;
  }

  return materialize;
}
