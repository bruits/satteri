import type {
  ContainerDirective,
  Custom,
  DescriptionDetails,
  DescriptionList,
  DescriptionTerm,
  DirectiveAttributes,
  HastNode,
  HastRaw,
  InlineMath,
  LeafDirective,
  MathNode,
  MdastNode,
  MdxFlowExpression,
  MdxFlowExpressionHast,
  MdxJsxAttributeNode,
  MdxJsxAttributeUnion,
  MdxJsxAttributeValueExpressionNode,
  MdxJsxExpressionAttributeNode,
  MdxJsxFlowElement,
  MdxJsxFlowElementHast,
  MdxJsxTextElement,
  MdxJsxTextElementHast,
  MdxTextExpression,
  MdxTextExpressionHast,
  MdxjsEsm,
  MdxjsEsmHast,
  Subscript,
  Superscript,
  TextDirective,
  Toml,
} from "../src/index.js";

type NodeType<T extends { type: string }> = T["type"];

export type _MdastNode = NodeType<MdastNode>;
export type _HastNode = NodeType<HastNode>;
export type _Custom = NodeType<Custom>;
export type _Toml = NodeType<Toml>;
export type _MathNode = NodeType<MathNode>;
export type _InlineMath = NodeType<InlineMath>;
export type _Superscript = NodeType<Superscript>;
export type _Subscript = NodeType<Subscript>;
export type _DescriptionList = NodeType<DescriptionList>;
export type _DescriptionTerm = NodeType<DescriptionTerm>;
export type _DescriptionDetails = NodeType<DescriptionDetails>;
export type _HastRaw = NodeType<HastRaw>;
export type _ContainerDirective = NodeType<ContainerDirective>;
export type _LeafDirective = NodeType<LeafDirective>;
export type _TextDirective = NodeType<TextDirective>;
export type _MdxJsxAttribute = NodeType<MdxJsxAttributeNode>;
export type _MdxJsxExpressionAttribute = NodeType<MdxJsxExpressionAttributeNode>;
export type _MdxJsxAttributeValueExpression = NodeType<MdxJsxAttributeValueExpressionNode>;
export type _MdxJsxAttributeUnion = NodeType<MdxJsxAttributeUnion>;
export type _MdxJsxFlowElement = NodeType<MdxJsxFlowElement>;
export type _MdxJsxTextElement = NodeType<MdxJsxTextElement>;
export type _MdxFlowExpression = NodeType<MdxFlowExpression>;
export type _MdxTextExpression = NodeType<MdxTextExpression>;
export type _MdxjsEsm = NodeType<MdxjsEsm>;
export type _MdxJsxFlowElementHast = NodeType<MdxJsxFlowElementHast>;
export type _MdxJsxTextElementHast = NodeType<MdxJsxTextElementHast>;
export type _MdxFlowExpressionHast = NodeType<MdxFlowExpressionHast>;
export type _MdxTextExpressionHast = NodeType<MdxTextExpressionHast>;
export type _MdxjsEsmHast = NodeType<MdxjsEsmHast>;
export type _DirectiveAttributes = DirectiveAttributes;
