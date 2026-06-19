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
export abstract class LazyChildResolver<TReader, TNode> {
  #handle: AnyHandle;
  #reader: TReader | null = null;
  #sealed = false;
  #epoch: number;

  constructor(handle: AnyHandle) {
    this.#handle = handle;
    this.#epoch = HANDLE_EPOCHS.get(handle) ?? 0;
  }

  protected abstract createReader(wire: Uint8Array): TReader;
  protected abstract materializeNode(reader: TReader, nodeId: number): TNode;
  protected abstract readParentId(reader: TReader, nodeId: number): number;
  protected abstract readChildIds(reader: TReader, nodeId: number): number[];

  /**
   * Mark the visitor pass over. Child ids were captured at match time; once
   * the pass's mutations land the arena is rebuilt and ids renumbered, so a
   * later snapshot would map those stale ids onto the wrong nodes (or out of
   * range). Failing loudly here beats silently wrong children.
   */
  seal(): void {
    this.#sealed = true;
  }

  /** Guards the `.children` getters: a first read after the pass is refused
   *  outright, even when no mutation landed — match-time ids cannot be trusted
   *  once the plugin no longer controls the tree. */
  assertUnsealed(): void {
    if (this.#sealed) {
      throw new Error(
        "Cannot read node content: this node was retained past its visitor pass " +
          "and the tree may have changed since; read it inside the visitor.",
      );
    }
  }

  #ensureReader(): TReader {
    if (this.#reader === null) {
      // A node id proves the tree was read in-pass, so a deferred snapshot is
      // still faithful as long as no command buffer rebuilt the arena since
      // match time (ids renumber on rebuild). An existing reader is always
      // safe: the snapshot is an immutable pre-mutation copy.
      if ((HANDLE_EPOCHS.get(this.#handle) ?? 0) !== this.#epoch) {
        throw new Error(
          "Cannot read node content: this node was retained past its visitor pass " +
            "and the tree has changed since; read it inside the visitor.",
        );
      }
      // The serialized buffer already carries each node's `data` blob (read
      // eagerly by the materializer), and the arena isn't mutated mid-visit —
      // so no separate lazy NAPI fetch is needed. This also keeps walk-path
      // children consistent with the fully materialized tree (no `data` key
      // when a node has none).
      this.#reader = this.createReader(serializeHandle(this.#handle));
    }
    return this.#reader;
  }

  /** Materialize one node for a child stub's first real-field read. */
  materializeOne(nodeId: number): TNode {
    return this.materializeNode(this.#ensureReader(), nodeId);
  }

  /** Arena id of `nodeId`'s parent in the pass snapshot, or undefined at the root. */
  parentIdOf(nodeId: number): number | undefined {
    this.assertUnsealed();
    const parentId = this.readParentId(this.#ensureReader(), nodeId);
    return parentId === NO_PARENT ? undefined : parentId;
  }

  /** Per-parent child-id→index maps, built lazily: null until a plugin calls
   *  `indexInParent` (most never do). Cache-safe because the snapshot is immutable. */
  #childIndexByParent: Map<number, Map<number, number>> | null = null;

  /** Index of `nodeId` within its parent's children in the pass snapshot,
   *  or undefined at the root. */
  indexInParent(nodeId: number): number | undefined {
    this.assertUnsealed();
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
