// Keep hot decoders separate so MDAST and HAST objects do not make their call sites polymorphic.

import { releaseCommandBuffer, type CommandBuffer } from "./command-buffer.js";
import { describeValue, isUsageError, usageError } from "./usage-errors.js";

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
  return usageError(
    `replaceNode: the document root takes a \`root\`, not ${describeValue(content)}. ` +
      'Pass { type: "root", children: [...] } to swap the document, ' +
      'or setField(root, "children", [...]) to swap only its children.',
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

const COMMAND_ERROR = "command error: ";

export function pluginLabel(plugin: object | null | undefined): string {
  const name = (plugin as { name?: unknown } | null | undefined)?.name;
  return `satteri: plugin "${typeof name === "string" ? name : "<anonymous>"}"`;
}

/** Runs one plugin's pass or the apply of its edits, naming that plugin in the errors Sätteri raises there. */
export function withPluginErrors<T>(plugin: object | null | undefined, run: () => T): T {
  try {
    const result = run();
    if (result instanceof Promise) {
      return result.catch((error: unknown) => {
        throw namePlugin(plugin, error);
      }) as T;
    }
    return result;
  } catch (error) {
    throw namePlugin(plugin, error);
  }
}

/** Errors a plugin throws itself pass through as they are. */
function namePlugin(plugin: object | null | undefined, error: unknown): unknown {
  if (!(error instanceof Error)) return error;
  let message: string;
  if (error.message.startsWith(COMMAND_ERROR)) message = error.message.slice(COMMAND_ERROR.length);
  else if (isUsageError(error)) message = error.message.replace(/^satteri: /, "");
  else return error;
  const Kind = error instanceof TypeError ? TypeError : Error;
  return new Kind(`${pluginLabel(plugin)}: ${message}`, { cause: error });
}

/** A wrapper is one parent node, never a list. */
export function requireWrapper(wrapper: unknown): void {
  if (wrapper !== null && typeof wrapper === "object" && !Array.isArray(wrapper)) return;
  throw usageError(`wrapNode: expected a parent node, not ${describeValue(wrapper)}`, TypeError);
}

/** Rejects what cannot be content before encoding, so the error points at the argument. */
export function requireContent(method: string, content: unknown): void {
  if (!Array.isArray(content)) {
    if (content !== null && typeof content === "object") return;
    throw usageError(
      `${method}: expected a node or an array of nodes, not ${describeValue(content)}`,
      TypeError,
    );
  }
  for (let i = 0; i < content.length; i++) {
    const item: unknown = content[i];
    if (item !== null && typeof item === "object" && !Array.isArray(item)) continue;
    throw usageError(
      `${method}: item ${i} of the array is ${describeValue(item)}; pass nodes, in one flat array`,
      TypeError,
    );
  }
}

/** A visitor's return, checked like a context method's content. */
export function requireVisitorResult(
  visited: { type: string },
  result: unknown,
  raw: boolean,
): void {
  if (!Array.isArray(result)) {
    if (!isReturnItem(result, raw)) throw visitorResultError(visited, describeValue(result), raw);
    return;
  }
  for (const item of result as unknown[]) {
    if (!isReturnItem(item, raw)) {
      throw visitorResultError(visited, `an array holding ${describeValue(item)}`, raw);
    }
  }
}

function isReturnItem(item: unknown, raw: boolean): boolean {
  if (item === null || typeof item !== "object" || Array.isArray(item)) return false;
  const shaped = item as { type?: unknown; raw?: unknown; rawHtml?: unknown };
  if (typeof shaped.type === "string") return true;
  return raw && (typeof shaped.raw === "string" || typeof shaped.rawHtml === "string");
}

function visitorResultError(visited: { type: string }, what: string, raw: boolean): Error {
  return usageError(
    `the "${visited.type}" visitor returned ${what}; a visitor returns a node, an array of ` +
      `nodes${raw ? ", { raw }" : ""}, or nothing`,
    TypeError,
  );
}

/** Joins node labels from Rust for a warning: "the paragraph at 3:1 and 2 more nodes". */
export function listNodes(labels: readonly string[]): string {
  if (labels.length === 1) return labels[0]!;
  if (labels.length <= 3) return `${labels.slice(0, -1).join(", ")} and ${labels.at(-1)!}`;
  return `${labels.slice(0, 2).join(", ")} and ${labels.length - 2} more nodes`;
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

/** A pass's removals, to warn once it ends when a plugin listed a node it removed, which stays removed. */
export class RemovalTracker<TNode> {
  #removed: Set<number> | undefined;
  #listed: Set<number> | undefined;
  /** Anchor id, then content, per insert. */
  #inserted: unknown[] | undefined;
  /** The last whole-list verdict, shared by the rest of that run of children. */
  #runSiblings: readonly unknown[] | undefined;
  #runParent = 0;
  #runStart = 0;
  #runWhole = false;
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

  /** An insert places what it names, so a removal of any of it wins whichever came first. */
  inserted(anchorId: number, content: unknown): void {
    (this.#inserted ??= []).push(anchorId, content);
  }

  /** A replacement or children list naming a node already removed cannot bring it back. */
  rebuilt(targetId: number, content: unknown): void {
    // Kept tiny so the common no-removal case inlines into every replacement.
    if (this.#removed !== undefined) this.#collect(this.#removed, targetId, content);
  }

  /** A visitor's return replaces the node it visited, and an empty array removes it. */
  returned(targetId: number, result: unknown): void {
    if (Array.isArray(result) && result.length === 0) this.removed(targetId);
    else this.rebuilt(targetId, result);
  }

  warn(plugin: object, labelNodes: (ids: number[]) => string[]): void {
    const removed = this.#removed;
    const inserted = this.#inserted;
    if (removed !== undefined && inserted !== undefined) {
      for (let i = 0; i < inserted.length; i += 2) {
        this.#collect(removed, inserted[i] as number, inserted[i + 1]);
      }
    }
    const listed = this.#listed;
    if (listed !== undefined && listed.size > 0) {
      const ids = [...listed].toSorted((a, b) => a - b);
      console.warn(
        `${pluginLabel(plugin)}: ${listNodes(labelNodes(ids))} ${ids.length === 1 ? "is" : "are"} ` +
          "named in new content after being removed, and a removed node stays removed. " +
          "To move a node, insert it without calling removeNode.",
      );
    }
  }

  #collect(removed: ReadonlySet<number>, editedId: number, content: unknown): void {
    this.#runSiblings = undefined;
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
          !this.#passesOnWholeList(id, editedId, siblings, index)
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

  /** Content for the edited node that holds a parent's original children whole and in order, as `{ ...node }` does, only passes a removed one on. */
  #passesOnWholeList(
    id: number,
    editedId: number,
    siblings: readonly unknown[] | undefined,
    index: number,
  ): boolean {
    const parentId = this.#resolver.parentIdOf(id);
    const ownIndex = this.#resolver.indexInParent(id);
    if (siblings === undefined || parentId === undefined || ownIndex === undefined) return false;
    if (parentId !== editedId && !this.#isInside(parentId, editedId)) return false;
    const start = index - ownIndex;
    if (
      siblings === this.#runSiblings &&
      parentId === this.#runParent &&
      start === this.#runStart
    ) {
      return this.#runWhole;
    }
    const original = this.#resolver.childIdsOf(parentId);
    const whole =
      start >= 0 &&
      start + original.length <= siblings.length &&
      original.every((childId, i) => {
        const sibling = siblings[start + i];
        return (
          sibling !== null &&
          typeof sibling === "object" &&
          this.#nodeId(sibling as TNode, this.#refs) === childId
        );
      });
    this.#runSiblings = siblings;
    this.#runParent = parentId;
    this.#runStart = start;
    this.#runWhole = whole;
    return whole;
  }

  #isInside(id: number, ancestorId: number): boolean {
    for (let cur = this.#resolver.parentIdOf(id); cur !== undefined; ) {
      if (cur === ancestorId) return true;
      cur = this.#resolver.parentIdOf(cur);
    }
    return false;
  }
}

/** Inserting a node next to or into itself would leave it in two places. */
export function selfInsertError(method: string): Error {
  return usageError(
    `${method}: a node cannot be inserted next to or into itself. Pass structuredClone(node) to insert a copy.`,
  );
}

/** The wrapper becomes a new parent, so an existing node would end up in two places. */
export function existingWrapperError(): Error {
  return usageError(
    "wrapNode: the wrapper must be a new node, not one from the tree. Pass structuredClone(node) to wrap in a copy of an existing one.",
  );
}

export function foreignContentError(): Error {
  return usageError(
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
    if (node === null || typeof node !== "object") {
      throw usageError(
        `${method}: expected a node from the tree, not ${describeValue(node)}`,
        TypeError,
      );
    }
    const id = nid(node, refs);
    if (id !== undefined && id !== FOREIGN_REF) return id;
    throw usageError(
      `${method}: this node is not in the tree being edited. Either it was ` +
        `built in JS, in which case pass it as new content (e.g. the second argument of ` +
        `insertAfter), or it was read from another tree (a different document, the mdast phase of ` +
        `this one, or an earlier compile), in which case match it again in this pass.`,
    );
  };
}

export function takeCommands(buffer: CommandBuffer): Uint8Array {
  // Copy before releasing: later passes reuse the backing storage.
  const bytes = buffer.length > 0 ? buffer.getBuffer().slice() : EMPTY_BYTES;
  releaseCommandBuffer(buffer);
  return bytes;
}
