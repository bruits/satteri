import { serializeHandle } from "#binding";
import type { AnyHandle } from "./handles.js";

/** Rebuild count per handle: bumped whenever a command buffer lands and
 *  renumbers the arena, invalidating ids captured before it. */
const HANDLE_EPOCHS = new WeakMap<AnyHandle, number>();

/** Record that `handle`'s arena was rebuilt. Resolvers created before the bump
 *  refuse to take a fresh snapshot afterwards (their ids are stale). */
export function markHandleMutated(handle: AnyHandle): void {
  HANDLE_EPOCHS.set(handle, (HANDLE_EPOCHS.get(handle) ?? 0) + 1);
}

/** Arena sentinel in the node struct's parent field: the node has no parent. */
const NO_PARENT = 0xffffffff;

/**
 * Lazy node materializer for the walk paths: serializes the handle's arena
 * once, on the first stub materialization, then materializes nodes from that
 * snapshot. Subclasses supply reader construction and per-node materialization
 * so the hot path stays free of per-node closures.
 */
/** Snapshot + materialized nodes for one `(handle, epoch)`. Immutable once
 *  built, so it is shared by every resolver of the same pass chain: nested
 *  matched containers dedup by id, and later read-only passes reuse the
 *  whole cache instead of re-serializing and re-materializing. */
export interface EpochCache<TReader, TNode> {
  epoch: number;
  reader: TReader;
  nodes: Map<number, TNode>;
}

export abstract class LazyChildResolver<TReader, TNode> {
  #handle: AnyHandle;
  #epoch: number;
  /** Strong pin: retained nodes keep their pass snapshot alive after later epochs evict the slot. */
  #cache: EpochCache<TReader, TNode> | undefined;

  constructor(handle: AnyHandle) {
    this.#handle = handle;
    this.#epoch = HANDLE_EPOCHS.get(handle) ?? 0;
  }

  protected abstract createReader(wire: Uint8Array): TReader;
  protected abstract materializeNode(reader: TReader, nodeId: number): TNode;
  protected abstract readParentId(reader: TReader, nodeId: number): number;
  protected abstract readChildIds(reader: TReader, nodeId: number): number[];
  /** Kind-specific `(handle → cache)` slot, supplied as a module-level WeakMap
   *  by each subclass so MDAST and HAST snapshots never collide. */
  protected abstract cacheSlot(): WeakMap<AnyHandle, EpochCache<TReader, TNode>>;

  #ensureCache(): EpochCache<TReader, TNode> {
    let cache = this.#cache;
    if (cache !== undefined) return cache;
    const slot = this.cacheSlot();
    cache = slot.get(this.#handle);
    if (cache !== undefined && cache.epoch === this.#epoch) {
      this.#cache = cache;
      return cache;
    }
    // A node id proves the tree was read in-pass, so a deferred snapshot is
    // still faithful as long as no command buffer mutated the arena since
    // match time. An existing same-epoch cache is always safe: the snapshot
    // is an immutable copy.
    if ((HANDLE_EPOCHS.get(this.#handle) ?? 0) !== this.#epoch) {
      throw new Error(
        "Cannot read node content: this node was retained past its visitor pass " +
          "and the tree has changed since. Read any field of the node during its " +
          "pass to pin the pass snapshot, or copy the data you need.",
      );
    }
    // The serialized buffer already carries each node's `data` blob (read
    // eagerly by the materializer), and the arena isn't mutated mid-visit —
    // so no separate lazy NAPI fetch is needed. This also keeps walk-path
    // children consistent with the fully materialized tree (no `data` key
    // when a node has none).
    cache = {
      epoch: this.#epoch,
      reader: this.createReader(serializeHandle(this.#handle)),
      nodes: new Map(),
    };
    this.#cache = cache;
    slot.set(this.#handle, cache);
    return cache;
  }

  #ensureReader(): TReader {
    return this.#ensureCache().reader;
  }

  /** Whether this pass's snapshot already exists; never takes one itself. */
  hasHotSnapshot(): boolean {
    if (this.#cache !== undefined) return true;
    const cache = this.cacheSlot().get(this.#handle);
    return cache !== undefined && cache.epoch === this.#epoch;
  }

  /** Materialize one node for a child stub's first real-field read, memoized
   *  per `(handle, epoch, id)` so overlapping subtrees and later passes share
   *  the same materialized objects. */
  materializeOne(nodeId: number): TNode {
    const cache = this.#ensureCache();
    let node = cache.nodes.get(nodeId);
    if (node === undefined) {
      node = this.materializeNode(cache.reader, nodeId);
      cache.nodes.set(nodeId, node);
    }
    return node;
  }

  /** Arena id of `nodeId`'s parent in the pass snapshot, or undefined at the root. */
  parentIdOf(nodeId: number): number | undefined {
    const parentId = this.readParentId(this.#ensureReader(), nodeId);
    return parentId === NO_PARENT ? undefined : parentId;
  }

  /** Per-parent child-id→index maps, built lazily: null until a plugin calls
   *  `indexInParent` (most never do). Cache-safe because the snapshot is immutable. */
  #childIndexByParent: Map<number, Map<number, number>> | null = null;

  /** Index of `nodeId` within its parent's children in the pass snapshot,
   *  or undefined at the root. */
  indexInParent(nodeId: number): number | undefined {
    const reader = this.#ensureReader();
    const parentId = this.readParentId(reader, nodeId);
    if (parentId === NO_PARENT) return undefined;
    const byParent = (this.#childIndexByParent ??= new Map());
    let indexById = byParent.get(parentId);
    if (indexById === undefined) {
      const map = new Map<number, number>();
      this.readChildIds(reader, parentId).forEach((id, i) => map.set(id, i));
      byParent.set(parentId, map);
      indexById = map;
    }
    return indexById.get(nodeId);
  }
}
