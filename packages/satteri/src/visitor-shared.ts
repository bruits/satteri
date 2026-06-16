/**
 * Cold/shape-stable helpers shared by the mdast and hast visitors. The
 * per-node hot decoders stay duplicated in each visitor on purpose: sharing
 * them would feed differently-shaped arguments through one call site and turn
 * it polymorphic.
 */

import type { CommandBuffer } from "./command-buffer.js";

const EMPTY_BYTES = new Uint8Array(0);

export function asArray<T>(value: T | T[]): T[] {
  return Array.isArray(value) ? value : [value];
}

/** Thrown when declarative replacement content can't be compiled to the
 *  structural op-stream — an unsupported node type (e.g. a bare `root`/`doctype`
 *  handed in as content) or an out-of-range numeric field. The op-stream is the
 *  only structural encoding, so this is a hard error rather than a fallback. */
export function unencodableContentError(content: unknown): Error {
  const type = (content as { type?: unknown } | null)?.type;
  return new Error(
    `satteri: cannot encode replacement content${typeof type === "string" ? ` of type "${type}"` : ""} ` +
      "into the structural op-stream — unsupported node type or out-of-range numeric field.",
  );
}

/**
 * Arena id for a node passed to a context method, via a per-kind `nid` lookup
 * (closure keeps each kind's call site monomorphic). Plugin-built nodes have
 * no id; without this check the id would coerce to 0 in the command buffer
 * and the mutation would silently target the document root.
 */
export function makeRequireNid<TNode>(
  nid: (node: TNode) => number | undefined,
): (node: TNode, method: string) => number {
  return (node, method) => {
    const id = nid(node);
    if (id === undefined) {
      throw new Error(
        `${method}: node has no arena id — it was built in JS, not read from this tree. ` +
          `Pass plugin-built nodes as new content (e.g. the second argument of insertAfter).`,
      );
    }
    return id;
  };
}

/** Concatenate the return-value and context command buffers for one pass,
 *  resetting both for reuse. */
export function mergeAndReset(
  returnBuffer: CommandBuffer,
  ctx: { getCommandBuffer(): CommandBuffer },
): { merged: Uint8Array; hasMutations: boolean } {
  const ctxCmdBuf = ctx.getCommandBuffer();
  // The common case — no mutations this pass — allocates nothing.
  if (returnBuffer.length === 0 && ctxCmdBuf.length === 0) {
    return { merged: EMPTY_BYTES, hasMutations: false };
  }
  const ctxBuf = ctxCmdBuf.getBuffer();
  const retBuf = returnBuffer.getBuffer();
  const merged = new Uint8Array(retBuf.length + ctxBuf.length);
  merged.set(retBuf, 0);
  merged.set(ctxBuf, retBuf.length);

  returnBuffer.reset();
  ctxCmdBuf.reset();
  return { merged, hasMutations: true };
}
