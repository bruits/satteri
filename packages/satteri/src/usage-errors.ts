// Errors and value rules for how plugins use the API, kept free of imports so the encoders can share them.

const usageErrors = new WeakSet<Error>();

/** An error in how a plugin used the API; the pass it surfaces from names the plugin. */
export function usageError(
  message: string,
  Kind: ErrorConstructor | TypeErrorConstructor = Error,
): Error {
  const error = new Kind(message);
  usageErrors.add(error);
  return error;
}

export function isUsageError(error: Error): boolean {
  return usageErrors.has(error);
}

export type MdxJsxAttributeValue =
  | string
  | null
  | { type: "mdxJsxAttributeValueExpression"; value: string };

/** The values an MDX JSX attribute takes, the same through every route that sets one. */
export function requireMdxAttributeValue(name: string, value: unknown): void {
  if (typeof value === "string" || value === null || value === undefined || value === true) return;
  if (
    typeof value === "object" &&
    (value as { type?: unknown }).type === "mdxJsxAttributeValueExpression" &&
    typeof (value as { value?: unknown }).value === "string"
  ) {
    return;
  }
  throw usageError(
    `MDX attribute "${name}" takes a string, null or true for a boolean attribute, or ` +
      `{ type: "mdxJsxAttributeValueExpression", value: "..." } for an expression, not ${describeValue(value)}`,
    TypeError,
  );
}

/** The values a directive attribute takes; in an attributes object, null and undefined leave the key out. */
export function requireDirectiveAttributeValue(
  name: string,
  value: unknown,
  inObject: boolean,
): void {
  if (typeof value === "string" || (inObject && (value === null || value === undefined))) return;
  throw usageError(
    `directive attribute "${name}" takes a string, not ${describeValue(value)}`,
    TypeError,
  );
}

/** The values a hast property takes, the same through every route that sets one. */
export function requireHastPropertyValue(name: string, value: unknown): void {
  switch (typeof value) {
    case "string":
    case "number":
    case "boolean":
    case "undefined":
      return;
    case "object":
      if (value === null) return;
      if (Array.isArray(value) && value.every(isTokenItem)) return;
  }
  throw usageError(
    `property "${name}" takes a string, number, boolean, null, or an array of strings and ` +
      `numbers, not ${describeValue(value)}`,
    TypeError,
  );
}

function isTokenItem(item: unknown): boolean {
  return (
    typeof item === "string" || typeof item === "number" || item === null || item === undefined
  );
}

/** Why `node` cannot become a node in the tree; `known` says its type exists but is never content. */
export function unbuildableNodeError(node: unknown, known: boolean): Error {
  if (node === null || typeof node !== "object" || Array.isArray(node)) {
    return usageError(`expected a node, not ${describeValue(node)}`, TypeError);
  }
  const type = (node as { type?: unknown }).type;
  if (typeof type !== "string" || type === "") {
    return usageError(`a node needs a "type" string, not ${describeValue(type)}`, TypeError);
  }
  if (type === "root") {
    return usageError(
      'a "root" node cannot be placed inside the tree; pass its children instead',
      TypeError,
    );
  }
  return usageError(
    known
      ? `${nodeOfType(type)} cannot be placed inside the tree`
      : `"${type}" is not a node type this tree holds`,
    TypeError,
  );
}

/** Fields every node has but no edit can set, with what to do instead. */
export function requireSettableField(method: string, key: string): void {
  if (key === "type") {
    throw usageError(
      `${method}: a node's "type" cannot change in place; replace the node with replaceNode`,
      TypeError,
    );
  }
  if (key === "position") {
    throw usageError(`${method}: "position" comes from the source and cannot be set`, TypeError);
  }
}

/** The siblings an insert lands between; the root has none. */
export function requireSiblingAnchor(method: string, isRoot: boolean): void {
  if (!isRoot) return;
  throw usageError(
    `${method}: the document root has no siblings; add to it with prependChild or appendChild`,
    TypeError,
  );
}

/** An element's tag name ends up in the HTML, so it cannot hold what would break the tag. */
export function requireTagName(tagName: unknown): void {
  if (typeof tagName === "string" && /^[^\s<>/="'`]+$/.test(tagName)) return;
  throw usageError(
    `an element's "tagName" is a name without spaces or <>/="' characters, not ${describeValue(tagName)}`,
    TypeError,
  );
}

/** An mdast heading's depth, the same through every route that sets one. */
export function requireHeadingDepth(depth: unknown): void {
  if (typeof depth === "number" && Number.isInteger(depth) && depth >= 1 && depth <= 6) return;
  throw usageError(
    `a heading's "depth" is an integer from 1 to 6, not ${describeValue(depth)}`,
    TypeError,
  );
}

/** `data` is stored as JSON, and its `hProperties` become hast properties, so both are checked here. */
export function dataJson(data: unknown): string {
  const hProperties = (data as { hProperties?: unknown } | null | undefined)?.hProperties;
  if (hProperties !== null && typeof hProperties === "object") {
    for (const name in hProperties) {
      requireHastPropertyValue(name, (hProperties as Record<string, unknown>)[name]);
    }
  }
  let json: string | undefined;
  try {
    json = JSON.stringify(data);
  } catch (error) {
    throw usageError(`"data" must be JSON-serializable: ${(error as Error).message}`, TypeError);
  }
  if (typeof json !== "string") throw usageError(`"data" must be JSON-serializable`, TypeError);
  return json;
}

/** `a "paragraph" node`, `an "element" node`. */
export function nodeOfType(type: string): string {
  return `${/^[aeiou]/i.test(type) ? "an" : "a"} "${type}" node`;
}

/** Names a value in an error message without dumping it. */
export function describeValue(value: unknown): string {
  switch (typeof value) {
    case "string":
      return JSON.stringify(value.length > 40 ? `${value.slice(0, 40)}...` : value);
    case "number":
    case "boolean":
    case "undefined":
      return String(value);
    case "bigint":
      return `${value}n`;
    case "function":
      return "a function";
    case "symbol":
      return "a symbol";
  }
  if (value === null) return "null";
  if (Array.isArray(value)) return "an array";
  const type = (value as { type?: unknown }).type;
  return typeof type === "string" ? nodeOfType(type) : "an object";
}
