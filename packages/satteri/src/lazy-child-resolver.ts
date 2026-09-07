import { serializeHandle } from "#binding";
import type { NodeRefs } from "./visitor-shared.js";
import type { AnyHandle } from "./handles.js";

// Arena rebuilds can renumber nodes, invalidating IDs captured by earlier passes.
const HANDLE_EPOCHS = new WeakMap<AnyHandle, number>();

// Registered cache slots let mutations evict snapshots that handles would otherwise retain.
const EPOCH_CACHE_SLOTS: WeakMap<AnyHandle, object>[] = [];

export function registerEpochCacheSlot<T extends object>(
  slot: WeakMap<AnyHandle, T>,
): WeakMap<AnyHandle, T> {
  EPOCH_CACHE_SLOTS.push(slot);
  return slot;
}

// Ownership is per handle; epoch checks separately reject stale IDs within the same handle.
const HANDLE_NODE_REFS = new WeakMap<AnyHandle, NodeRefs>();

function nodeRefsOfHandle(handle: AnyHandle): NodeRefs {
  let refs = HANDLE_NODE_REFS.get(handle);
  if (refs === undefined) {
    refs = new WeakMap();
    HANDLE_NODE_REFS.set(handle, refs);
  }
  return refs;
}

export function markHandleMutated(handle: AnyHandle): void {
  HANDLE_EPOCHS.set(handle, (HANDLE_EPOCHS.get(handle) ?? 0) + 1);
  for (const slot of EPOCH_CACHE_SLOTS) {
    slot.delete(handle);
  }
}

const NO_PARENT = 0xffffffff;

// Immutable snapshots can be shared by resolvers until the next arena mutation.
export interface EpochCache<TReader> {
  epoch: number;
  reader: TReader;
}

// Subclasses keep the per-node materialization path free of closures.
export abstract class LazyChildResolver<TReader, TNode> {
  #handle: AnyHandle;
  #epoch: number;
  readonly refs: NodeRefs;
  /** Strong pin: retained nodes keep their pass snapshot alive after later epochs evict the slot. */
  #cache: EpochCache<TReader> | undefined;

  constructor(handle: AnyHandle) {
    this.#handle = handle;
    this.#epoch = HANDLE_EPOCHS.get(handle) ?? 0;
    this.refs = nodeRefsOfHandle(handle);
  }

  protected abstract createReader(wire: Uint8Array): TReader;
  protected abstract materializeNode(reader: TReader, nodeId: number, refs: NodeRefs): TNode;
  protected abstract readParentId(reader: TReader, nodeId: number): number;
  protected abstract readChildIds(reader: TReader, nodeId: number): number[];
  protected abstract cacheSlot(): WeakMap<AnyHandle, EpochCache<TReader>>;

  #ensureCache(): EpochCache<TReader> {
    let cache = this.#cache;
    if (cache !== undefined) return cache;
    const slot = this.cacheSlot();
    cache = slot.get(this.#handle);
    if (cache !== undefined && cache.epoch === this.#epoch) {
      this.#cache = cache;
      return cache;
    }
    // Snapshot only within the captured epoch; later arenas may assign these IDs to different nodes.
    if ((HANDLE_EPOCHS.get(this.#handle) ?? 0) !== this.#epoch) {
      throw new Error(
        "Cannot read node content: this node was retained past its visitor pass " +
          "and the tree has changed since. Reading a child node's field (or calling " +
          "ctx.parent()) during the pass pins the pass snapshot; eager fields like " +
          "tagName or properties do not. Or copy the data you need before the pass ends.",
      );
    }
    cache = {
      epoch: this.#epoch,
      reader: this.createReader(serializeHandle(this.#handle)),
    };
    this.#cache = cache;
    slot.set(this.#handle, cache);
    return cache;
  }

  #ensureReader(): TReader {
    return this.#ensureCache().reader;
  }

  hasHotSnapshot(): boolean {
    if (this.#cache !== undefined) return true;
    const cache = this.cacheSlot().get(this.#handle);
    return cache !== undefined && cache.epoch === this.#epoch;
  }

  materializeOne(nodeId: number): TNode {
    return this.materializeNode(this.#ensureCache().reader, nodeId, this.refs);
  }

  parentIdOf(nodeId: number): number | undefined {
    const parentId = this.readParentId(this.#ensureReader(), nodeId);
    return parentId === NO_PARENT ? undefined : parentId;
  }

  // Immutable snapshots make cached child indices safe across repeated parent lookups.
  #childIndexByParent: Map<number, Map<number, number>> | null = null;

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
