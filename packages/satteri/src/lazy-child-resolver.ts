import { serializeHandle } from "#binding";
import type { AnyHandle } from "./handles.js";

/**
 * Lazy child materializer for the walk paths: serializes the handle's arena
 * once, on the first child materialization, then materializes children from
 * that snapshot. Subclasses supply reader construction and per-node
 * materialization so the hot path stays free of per-node closures.
 */
export abstract class LazyChildResolver<TReader, TNode> {
  #handle: AnyHandle;
  #reader: TReader | null = null;
  #sealed = false;

  constructor(handle: AnyHandle) {
    this.#handle = handle;
  }

  protected abstract createReader(wire: Uint8Array): TReader;
  protected abstract materializeNode(reader: TReader, nodeId: number): TNode;

  /**
   * Mark the visitor pass over. Child ids were captured at match time; once
   * the pass's mutations land the arena is rebuilt and ids renumbered, so a
   * later snapshot would map those stale ids onto the wrong nodes (or out of
   * range). Failing loudly here beats silently wrong children.
   */
  seal(): void {
    this.#sealed = true;
  }

  materializeChildren(childIds: number[]): TNode[] {
    if (this.#sealed) {
      throw new Error(
        "Cannot read `.children`: this node was retained past its visitor pass " +
          "and the tree may have changed since; read `.children` inside the visitor.",
      );
    }
    // The serialized buffer already carries each node's `data` blob (read
    // eagerly by the materializer), and the arena isn't mutated mid-visit — so
    // no separate lazy NAPI fetch is needed. This also keeps walk-path
    // children consistent with the fully materialized tree (no `data` key
    // when a node has none).
    this.#reader ??= this.createReader(serializeHandle(this.#handle));
    const reader = this.#reader;
    return childIds.map((id) => this.materializeNode(reader, id));
  }
}
