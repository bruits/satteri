/**
 * Cold/shape-stable helpers shared by the mdast and hast visitors. The
 * per-node hot decoders stay duplicated in each visitor on purpose: sharing
 * them would feed differently-shaped arguments through one call site and turn
 * it polymorphic.
 */

import type { CommandBuffer } from "./command-buffer.js";

/** Plugin-level configuration, set via `options` on a plugin definition. */
export interface PluginOptions {
  /**
   * Set to `true` if any visitor reads `node.position`. When no plugin in the
   * pipeline opts in, source-position tracking is skipped during parsing
   * (~15% faster parse), and `node.position` is `undefined`.
   */
  position?: boolean;
}

const EMPTY_BYTES = new Uint8Array(0);

export const ROOT_NODE_ID = 0;

export function rootReplacementError(content: unknown): Error {
  const type = (content as { type?: unknown } | null)?.type;
  return new Error(
    `satteri: replaceNode on the document root takes a \`root\`${
      typeof type === "string" ? `, not "${type}"` : ""
    }. Pass { type: "root", children: [...] } to swap the document, ` +
      'or setProperty(root, "children", [...]) to swap only its children.',
  );
}

/** Hooks subscribe to node 0 by node type, so a document left headed by anything
 *  but a `root` silently stops firing them, in this phase and every later one. */
export function requireRootReplacement<T>(content: T): T {
  if ((content as { type?: unknown } | null)?.type === "root") return content;
  throw rootReplacementError(content);
}

/** A splice by id has no answer when the content would have to contain itself. */
export function reuseAncestorError(op: string): Error {
  return new Error(
    `satteri: ${op} was passed a node that contains its own insertion point, so it cannot be ` +
      "spliced there. Pass structuredClone(node) to insert a copy instead.",
  );
}

export function reuseCycleError(op: string): Error {
  return new Error(
    `satteri: ${op} and an earlier insert each reuse the other's node, so neither can be ` +
      'resolved. To reorder siblings, use setProperty(parent, "children", [...]) with the ' +
      "children in the order you want.",
  );
}

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

/** Per handle, so membership alone proves ownership and the read path needs no marker of its own. */
export type NodeRefs = WeakMap<object, number>;

/** Separates "belongs to another tree" from "never had an id"; arena ids are never negative. */
export const FOREIGN_REF = -1;

/** `_refs` rides on the prototype, surviving neither a spread copy nor an object literal. */
export function crossPipelineForeign(node: object): number | undefined {
  // Probed before the own-property call: plugin-built content misses here, and that is the hot case.
  if ((node as { _refs?: unknown })._refs === undefined) return undefined;
  return Object.hasOwn(node, "_refs") ? undefined : FOREIGN_REF;
}

/**
 * Arena id for a node passed to a context method, via a per-kind `nid` lookup
 * (closure keeps each kind's call site monomorphic) resolved against the
 * edited tree's own refs. Plugin-built nodes have no id; without this check
 * the id would coerce to 0 in the command buffer and the mutation would
 * silently target the document root. A node read from another tree has an id
 * that is meaningless here, and one that happens to be in range would edit an
 * unrelated node.
 */
export function makeRequireNid<TNode>(
  nid: (node: TNode, refs: NodeRefs) => number | undefined,
): (node: TNode, method: string, refs: NodeRefs) => number {
  return (node, method, refs) => {
    const id = nid(node, refs);
    if (id !== undefined && id !== FOREIGN_REF) return id;
    throw new Error(
      `${method}: invalid node id — this node has no id in the tree being edited. Either it was ` +
        `built in JS, in which case pass it as new content (e.g. the second argument of ` +
        `insertAfter), or it was read from another tree (a different document, the mdast phase of ` +
        `this one, or an earlier compile), in which case match it again in this pass.`,
    );
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
