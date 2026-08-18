/**
 * Shared materializer machinery for the HAST and MDAST flavors: per-reader
 * node memo, lazy `children` descriptors, and the frozen-mode (plugin walk
 * path) freeze rules.
 */

import type { Position } from "unist";
import { deepFreeze } from "./freeze.js";
import type { NodeRefs } from "./visitor-shared.js";

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
  /** Frozen mode only; mutable mode builds a per-node descriptor instead. */
  children: PropertyDescriptor | undefined;
  frozen: boolean;
  /** Frozen mode only: the edited tree's refs, which these nodes join as proof of ownership. */
  refs: NodeRefs | undefined;
}

export interface MaterializerSpec<TReader extends MaterializerReader, TNode extends object> {
  /** Function name used in error/warning messages (e.g. "materializeHastNode"). */
  label: string;
  /** Node-type tag -> canonical AST name (the generated `TYPE_NAMES`). */
  typeNames: Readonly<Record<number, string>>;
  /** Whether `node` carries `children`. Takes the built node because mdast
   *  `custom` decides leafness per node rather than per type. */
  hasChildren(nodeType: number, node: TNode, reader: TReader, nodeId: number): boolean;
  /**
   * Install the type-specific eager fields on `node`. Must not install
   * `children`, `position`, `data`, or `_nodeId`, and must not freeze:
   * the shared machinery owns all of those.
   */
  populate(node: TNode, reader: TReader, nodeId: number, nodeType: number): void;
}

/**
 * Build a memoizing materializer, memoized per `(reader, id)`.
 *
 * `node` materializes one node with lazy `children`; `frozen` (the plugin walk
 * path) deep-freezes every node at construction so plugins cannot corrupt the
 * shared cache. `tree` materializes a whole tree eagerly, which is what the
 * step-by-step API wants: it asked for the tree, so laziness would only add
 * per-node accessor overhead.
 */
export function createMaterializer<TReader extends MaterializerReader, TNode extends object>(
  spec: MaterializerSpec<TReader, TNode>,
): {
  node: (reader: TReader, nodeId: number, frozen?: boolean, refs?: NodeRefs) => TNode;
  tree: (reader: TReader, rootId: number) => TNode;
} {
  const readerCaches = new WeakMap<TReader, ReaderCache<TNode>>();

  function materialize(reader: TReader, nodeId: number, frozen = false, refs?: NodeRefs): TNode {
    const cache = readerCache(reader, frozen, refs);
    let node = cache.nodes.get(nodeId);
    if (node === undefined) {
      node = buildNode(reader, cache, nodeId, reader.getNodeType(nodeId));
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
          const ids = reader.getChildIds(nodeId);
          const built = new Array<TNode>(ids.length);
          let i = 0;
          for (const childId of ids) built[i++] = materialize(reader, childId, true);
          value = Object.freeze(built);
          cache.childLists.set(nodeId, value);
        }
        return value;
      },
      configurable: true,
      enumerable: true,
    };
  }

  /** Mutable-mode `children`: self-replacing with a plain writable array on
   *  first read. The id is captured rather than stored on the node, so a
   *  materialized tree carries no marker for `toEqual` or a spread to find. */
  function mutableChildrenDescriptor(reader: TReader, nodeId: number): PropertyDescriptor {
    return {
      get(this: TNode): TNode[] {
        const ids = reader.getChildIds(nodeId);
        const value = new Array<TNode>(ids.length);
        let i = 0;
        for (const childId of ids) value[i++] = materialize(reader, childId);
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

  function readerCache(reader: TReader, frozen: boolean, refs: NodeRefs | undefined): ReaderCache<TNode> {
    let cache = readerCaches.get(reader);
    if (cache === undefined) {
      cache = {
        nodes: new Map(),
        childLists: new Map(),
        children: undefined,
        frozen,
        refs,
      };
      if (frozen) cache.children = frozenChildrenDescriptor(reader, cache);
      readerCaches.set(reader, cache);
    }
    if (cache.frozen !== frozen) {
      throw new Error(`${spec.label}: a reader cannot mix frozen and mutable materialization`);
    }
    return cache;
  }

  function buildNode(
    reader: TReader,
    cache: ReaderCache<TNode>,
    nodeId: number,
    nodeType: number,
    eager = false,
  ): TNode {
    const typeName = spec.typeNames[nodeType] ?? `unknown(${nodeType})`;

    // Plain object, not a class: unified's `assertNode` rejects any other prototype.
    const node = { type: typeName } as unknown as TNode;
    const position = reader.getPosition(nodeId);
    if (position !== undefined) {
      (node as { position?: Position }).position = position;
    }

    if (cache.frozen) {
      // Non-enumerable so `nid()` never trusts an id that a spread copied.
      cache.refs?.set(node, nodeId);
      Object.defineProperty(node, "_nodeId", {
        value: nodeId,
        writable: false,
        configurable: true,
        enumerable: false,
      });
    }

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

    if (!eager && spec.hasChildren(nodeType, node, reader, nodeId)) {
      Object.defineProperty(
        node,
        "children",
        cache.children ?? mutableChildrenDescriptor(reader, nodeId),
      );
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

  /**
   * Iterative on purpose: recursing to full document depth overflows on deeply
   * nested input, and nothing else here descends more than one level.
   *
   * Breadth-first over the parents still to fill, so every node is built and
   * every child list decoded exactly once, and a child is wired into its
   * parent's array as it is built rather than in a second pass.
   */
  function fillTree(reader: TReader, cache: ReaderCache<TNode>, rootId: number): TNode {
    const rootType = reader.getNodeType(rootId);
    const root = buildNode(reader, cache, rootId, rootType, true);
    if (!spec.hasChildren(rootType, root, reader, rootId)) return root;

    const parents: TNode[] = [root];
    const parentIds: number[] = [rootId];
    for (let p = 0; p < parentIds.length; p++) {
      const parent = parents[p];
      const parentId = parentIds[p];
      if (parent === undefined || parentId === undefined) continue;
      const ids = reader.getChildIds(parentId);
      const kids = new Array<TNode>(ids.length);
      for (let i = 0; i < ids.length; i++) {
        const childId = ids[i];
        if (childId === undefined) continue;
        const childType = reader.getNodeType(childId);
        const child = buildNode(reader, cache, childId, childType, true);
        kids[i] = child;
        if (spec.hasChildren(childType, child, reader, childId)) {
          parents.push(child);
          parentIds.push(childId);
        }
      }
      // Assignment beats defineProperty here; `eager` left `children` uninstalled.
      (parent as { children?: TNode[] }).children = kids;
    }

    return root;
  }

  /** Whole tree at once: the caller will walk it, so lazy accessors cost more than they defer. */
  function materializeTree(reader: TReader, rootId: number): TNode {
    return fillTree(reader, readerCache(reader, false, undefined), rootId);
  }

  return { node: materialize, tree: materializeTree };
}
