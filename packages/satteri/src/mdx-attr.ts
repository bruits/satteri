import { restorePhantomSpaces } from "./phantom.js";
import type { MdxJsxAttributeUnion } from "./types.js";

export function decodeMdxJsxAttr(kind: number, name: string, value: string): MdxJsxAttributeUnion {
  switch (kind) {
    case 0:
      return { type: "mdxJsxAttribute", name, value: null };
    case 1:
      return { type: "mdxJsxAttribute", name, value };
    case 2:
      return {
        type: "mdxJsxAttribute",
        name,
        value: { type: "mdxJsxAttributeValueExpression", value: restorePhantomSpaces(value) },
      };
    default:
      return { type: "mdxJsxExpressionAttribute", value: restorePhantomSpaces(value) };
  }
}
