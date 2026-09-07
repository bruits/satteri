// Keep hot decoders separate so MDAST and HAST objects do not make their call sites polymorphic.

import { releaseCommandBuffer, type CommandBuffer } from "./command-buffer.js";

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

// Hooks require a root node at ID 0; replacing it with another type would silently disable them.
export function requireRootReplacement<T>(content: T): T {
  if ((content as { type?: unknown } | null)?.type === "root") return content;
  throw rootReplacementError(content);
}

export function asArray<T>(value: T | T[]): T[] {
  return Array.isArray(value) ? value : [value];
}

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

// Reject missing or foreign IDs before a command can target node 0 or an unrelated tree’s node.
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

export function collectCommands(
  returnBuffer: CommandBuffer,
  contextBuffer: CommandBuffer,
): Uint8Array {
  let merged = EMPTY_BYTES;
  const length = returnBuffer.length + contextBuffer.length;
  if (length > 0) {
    merged = new Uint8Array(length);
    merged.set(returnBuffer.getBuffer());
    merged.set(contextBuffer.getBuffer(), returnBuffer.length);
  }
  // Copy before releasing: later passes reuse the backing storage.
  releaseCommandBuffer(returnBuffer);
  releaseCommandBuffer(contextBuffer);
  return merged;
}
