// Re-export standard mdast types so consumers don't need direct @types/mdast deps
export type {
  Root,
  Nodes,
  RootContent,
  Blockquote,
  Break,
  Code,
  Definition,
  Delete,
  Emphasis,
  FootnoteDefinition,
  FootnoteReference,
  Heading,
  Html,
  Image,
  ImageReference,
  InlineCode,
  Link,
  LinkReference,
  List,
  ListItem,
  Paragraph,
  Strong,
  Table,
  TableRow,
  TableCell,
  Text,
  ThematicBreak,
  Yaml,
  Data,
  Literal,
  Parent,
} from "mdast";

// MDX mdast types
export type {
  MdxJsxFlowElement,
  MdxJsxTextElement,
  MdxJsxAttribute,
  MdxJsxExpressionAttribute,
  MdxJsxAttributeValueExpression,
} from "mdast-util-mdx-jsx";
export type { MdxFlowExpression, MdxTextExpression } from "mdast-util-mdx-expression";
export type { MdxjsEsm } from "mdast-util-mdxjs-esm";

// Custom extension types
export type { Toml, MathNode, InlineMath } from "./types.js";
