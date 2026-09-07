import { deepFreeze } from "./freeze.js";
import type { Node } from "unist";
import type { NodeRefs } from "./visitor-shared.js";

export interface MaterializerReader {
  getNodeType(nodeId: number): number;
  getChildIds(nodeId: number): number[];
  getChildrenCount(nodeId: number): number;
  getNodeData(nodeId: number): string | null;
}

// Identity-based plugin deduplication requires one object per reader and node ID.
interface ReaderCache<TNode extends Node> {
  nodes: Map<number, TNode>;
  childLists: Map<number, readonly TNode[]>;
  children: PropertyDescriptor | undefined;
  frozen: boolean;
  refs: NodeRefs | undefined;
}

export interface MaterializerSpec<TReader extends MaterializerReader, TNode extends Node> {
  label: string;
  typeNames: Readonly<Record<number, string>>;
  // Custom MDAST nodes determine leafness from their fields, not just their type.
  hasChildren(nodeType: number, node: TNode, reader: TReader, nodeId: number): boolean;
  // The materializer owns children, data, identity, and freezing; populate only eager type fields.
  populate(node: TNode, reader: TReader, nodeId: number, nodeType: number): void;
}

export function installNodeData(
  node: Node,
  rawData: string | null,
  label: string,
  nodeId: number,
): void {
  if (rawData === null) return;
  try {
    const parsed: unknown = JSON.parse(rawData);
    if (parsed && typeof parsed === "object" && Object.keys(parsed).length > 0) {
      node.data = parsed;
    }
  } catch (err) {
    if (process.env.NODE_ENV !== "production") {
      console.warn(`${label}: malformed node_data for nodeId=${nodeId}`, err);
    }
  }
}

export function createMaterializer<TReader extends MaterializerReader, TNode extends Node>(
  spec: MaterializerSpec<TReader, TNode>,
) {
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

  // A frozen node cannot replace its children getter, so cache the array separately.
  function frozenChildrenDescriptor(
    reader: TReader,
    cache: ReaderCache<TNode>,
  ): PropertyDescriptor {
    return {
      get(this: TNode & { _nodeId: number }): readonly TNode[] {
        const nodeId = this._nodeId;
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

  // Capture the ID so mutable trees expose no arena identity marker.
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

  function readerCache(
    reader: TReader,
    frozen: boolean,
    refs: NodeRefs | undefined,
  ): ReaderCache<TNode> {
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
  ): TNode {
    const typeName = spec.typeNames[nodeType] ?? `unknown(${nodeType})`;

    // Unified's assertNode requires a plain-object prototype.
    const node = { type: typeName } as TNode;

    // Populate before _nodeId so both materialization paths share V8 hidden classes.
    spec.populate(node, reader, nodeId, nodeType);

    if (cache.frozen) {
      // Non-enumerable IDs prevent spread copies from impersonating arena nodes.
      cache.refs?.set(node, nodeId);
      Object.defineProperty(node, "_nodeId", {
        value: nodeId,
        writable: false,
        configurable: true,
        enumerable: false,
      });
    }

    installNodeData(node, reader.getNodeData(nodeId), spec.label, nodeId);

    if (spec.hasChildren(nodeType, node, reader, nodeId)) {
      Object.defineProperty(
        node,
        "children",
        cache.children ?? mutableChildrenDescriptor(reader, nodeId),
      );
    }

    if (cache.frozen) {
      // Inspect descriptors to avoid triggering lazy children getters while freezing.
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

  return { node: materialize };
}
