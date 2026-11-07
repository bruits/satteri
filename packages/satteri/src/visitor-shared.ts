// Keep hot decoders separate so MDAST and HAST objects do not make their call sites polymorphic.

import { releaseCommandBuffer, type CommandBuffer } from "./command-buffer.js";
import type { MdxJsxAttributeUnion } from "./types.js";

/** Fields representable by named-value commands or explicit structured-field handlers. */
type ScalarFieldValue = string | number | boolean | null;

export type SettableFieldKey<N, IsMdast extends boolean = false> = {
  [K in keyof N & string]-?: K extends "type"
    ? never
    : Exclude<N[K], undefined> extends ScalarFieldValue
      ? K
      : K extends "attributes"
        ? (
            IsMdast extends true
              ? N extends {
                  type: "mdxJsxFlowElement" | "mdxJsxTextElement";
                  attributes: unknown[];
                }
                ? true
                : N extends {
                      type: "containerDirective" | "leafDirective" | "textDirective";
                      attributes?: unknown;
                    }
                  ? true
                  : false
              : N extends {
                    type: "mdxJsxFlowElement" | "mdxJsxTextElement";
                    attributes: unknown[];
                  }
                ? true
                : false
          ) extends true
          ? K
          : never
        : K extends "properties"
          ? IsMdast extends false
            ? N extends { type: "element"; properties: object }
              ? K
              : never
            : never
          : never;
}[keyof N & string];

/** Return a replacement copy with one named MDX JSX attribute upserted at the end. */
export function withMdxJsxAttribute<
  N extends { attributes: MdxJsxAttributeUnion[] | null | undefined },
>(node: N, name: string, value: unknown): N {
  const attributes: MdxJsxAttributeUnion[] = [...(node.attributes ?? [])];
  const index = attributes.findIndex(
    (attribute) => attribute.type === "mdxJsxAttribute" && attribute.name === name,
  );
  if (index !== -1) attributes.splice(index, 1);
  const attrValue =
    value === true || value === null || value === undefined
      ? null
      : typeof value === "string"
        ? value
        : Array.isArray(value)
          ? value.join(" ")
          : String(value);
  attributes.push({ type: "mdxJsxAttribute", name, value: attrValue });
  return { ...node, attributes };
}

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

interface ChildResolver {
  parentIdOf(id: number): number | undefined;
  indexInParent(id: number): number | undefined;
  childIdsOf(id: number): readonly number[];
}

/** A pass's removals, to warn when a plugin lists a node it removed; listing one never brings it back. */
export class RemovalTracker<TNode> {
  #removed: Set<number> | undefined;
  #listed: Set<number> | undefined;
  /** Anchor id, then content, per sibling insert; only once the pass ends is it known which anchors went away. */
  #insertsBeside: unknown[] | undefined;
  readonly #nodeId: (node: TNode, refs: NodeRefs) => number | undefined;
  readonly #refs: NodeRefs;
  readonly #resolver: ChildResolver;

  constructor(
    nodeId: (node: TNode, refs: NodeRefs) => number | undefined,
    refs: NodeRefs,
    resolver: ChildResolver,
  ) {
    this.#nodeId = nodeId;
    this.#refs = refs;
    this.#resolver = resolver;
  }

  removed(id: number): void {
    (this.#removed ??= new Set()).add(id);
  }

  insertedBeside(anchorId: number, content: unknown): void {
    (this.#insertsBeside ??= []).push(anchorId, content);
  }

  /** Notes the removed nodes inside `targetId` that content rebuilding it lists. */
  rebuilt(targetId: number, content: unknown): void {
    // Kept tiny so the common no-removal case inlines into every replacement.
    if (this.#removed !== undefined) this.#collect(this.#removed, targetId, content);
  }

  /** Checks an async visitor's result on settling, against the removals made by then. */
  rebuiltOnSettling<T>(targetId: number, result: Promise<T>): Promise<T> {
    return result.then((settled) => {
      this.rebuilt(targetId, settled);
      return settled;
    });
  }

  listedCount(): number {
    const removed = this.#removed;
    if (removed === undefined) return 0;
    // Content beside a removed node takes its place, whichever was queued first.
    const inserts = this.#insertsBeside ?? [];
    for (let i = 0; i < inserts.length; i += 2) {
      const anchorId = inserts[i] as number;
      if (removed.has(anchorId)) this.#collect(removed, anchorId, inserts[i + 1]);
    }
    return this.#listed?.size ?? 0;
  }

  #collect(removed: ReadonlySet<number>, targetId: number, content: unknown): void {
    const listed = (this.#listed ??= new Set());
    const visit = (
      node: unknown,
      siblings: readonly unknown[] | undefined,
      index: number,
    ): void => {
      if (node === null || typeof node !== "object") return;
      const id = this.#nodeId(node as TNode, this.#refs);
      if (id !== undefined) {
        if (
          id !== FOREIGN_REF &&
          removed.has(id) &&
          !this.#passesOnWholeList(id, siblings, index) &&
          isInside(this.#resolver, id, targetId)
        ) {
          listed.add(id);
        }
        return;
      }
      const children = (node as { children?: unknown }).children;
      if (Array.isArray(children)) children.forEach((child, i) => visit(child, children, i));
    };
    if (Array.isArray(content)) content.forEach((item, i) => visit(item, content, i));
    else visit(content, undefined, 0);
  }

  /** A list holding its parent's original children whole and in order, as `{ ...node }` does, only passes a removed one on. */
  #passesOnWholeList(id: number, siblings: readonly unknown[] | undefined, index: number): boolean {
    const parentId = this.#resolver.parentIdOf(id);
    const ownIndex = this.#resolver.indexInParent(id);
    if (siblings === undefined || parentId === undefined || ownIndex === undefined) return false;
    const original = this.#resolver.childIdsOf(parentId);
    const start = index - ownIndex;
    if (start < 0 || start + original.length > siblings.length) return false;
    return original.every((childId, i) => {
      const sibling = siblings[start + i];
      return (
        sibling !== null &&
        typeof sibling === "object" &&
        this.#nodeId(sibling as TNode, this.#refs) === childId
      );
    });
  }
}

function isInside(resolver: ChildResolver, id: number, ancestorId: number): boolean {
  for (let cur = resolver.parentIdOf(id); cur !== undefined; cur = resolver.parentIdOf(cur)) {
    if (cur === ancestorId) return true;
  }
  return false;
}

export function warnIfRemovalsListed(count: number, plugin: object, kind: "mdast" | "hast"): void {
  if (count === 0) return;
  const name = (plugin as { name?: unknown }).name;
  console.warn(
    `satteri: plugin "${typeof name === "string" ? name : "<anonymous>"}" listed ${count} ` +
      `${kind} node(s) it removed, and a removed node stays removed. Leave ` +
      `${count === 1 ? "it" : "them"} out of the list, or do not remove a node you want to keep.`,
  );
}

export function foreignContentError(): Error {
  return new Error(
    "satteri: content contains a node from another tree or pass; use structuredClone(node) to insert a detached copy.",
  );
}

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
