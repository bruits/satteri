// Re-export standard hast types so consumers don't need direct @types/hast deps
export type {
  Root,
  Element,
  Text,
  Comment,
  Doctype,
  Properties,
  Nodes,
  RootContent,
  ElementContent,
  Data,
  Literal,
  Parent,
} from "hast";

// MDX hast types
export type {
  MdxJsxFlowElementHast,
  MdxJsxTextElementHast,
  MdxJsxAttribute,
  MdxJsxExpressionAttribute,
  MdxJsxAttributeValueExpression,
} from "mdast-util-mdx-jsx";
export type { MdxFlowExpressionHast, MdxTextExpressionHast } from "mdast-util-mdx-expression";
export type { MdxjsEsmHast } from "mdast-util-mdxjs-esm";

// Custom extension types
export type { HastRaw } from "./types.js";
