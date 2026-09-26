// Local MDX interfaces avoid dependencies on packages needed only for their type definitions.

import type {
  Data as HastData,
  ElementContent,
  Literal as HastLiteral,
  Parent as HastParent,
} from "hast";
import type {
  BlockContent,
  Data as MdastData,
  DefinitionContent,
  Literal as MdastLiteral,
  Parent as MdastParent,
  PhrasingContent,
} from "mdast";
import type { Data, Node } from "unist";

export interface MdxJsxAttributeValueExpression extends Node {
  type: "mdxJsxAttributeValueExpression";
  value: string;
  data?: MdxJsxAttributeValueExpressionData | undefined;
}

export interface MdxJsxAttributeValueExpressionData extends Data {}

export interface MdxJsxExpressionAttribute extends Node {
  type: "mdxJsxExpressionAttribute";
  value: string;
  data?: MdxJsxExpressionAttributeData | undefined;
}

export interface MdxJsxExpressionAttributeData extends Data {}

export interface MdxJsxAttribute extends Node {
  type: "mdxJsxAttribute";
  name: string;
  value?: MdxJsxAttributeValueExpression | string | null | undefined;
  data?: MdxJsxAttributeData | undefined;
}

export interface MdxJsxAttributeData extends Data {}

export interface MdxJsxFlowElement extends MdastParent {
  type: "mdxJsxFlowElement";
  name: string | null;
  attributes: Array<MdxJsxAttribute | MdxJsxExpressionAttribute>;
  children: Array<BlockContent | DefinitionContent>;
  data?: MdxJsxFlowElementData | undefined;
}

export interface MdxJsxFlowElementData extends MdastData {}

export interface MdxJsxTextElement extends MdastParent {
  type: "mdxJsxTextElement";
  name: string | null;
  attributes: Array<MdxJsxAttribute | MdxJsxExpressionAttribute>;
  children: PhrasingContent[];
  data?: MdxJsxTextElementData | undefined;
}

export interface MdxJsxTextElementData extends MdastData {}

export interface MdxJsxFlowElementHast extends HastParent {
  type: "mdxJsxFlowElement";
  name: string | null;
  attributes: Array<MdxJsxAttribute | MdxJsxExpressionAttribute>;
  children: ElementContent[];
  data?: MdxJsxFlowElementHastData | undefined;
}

export interface MdxJsxFlowElementHastData extends HastData {}

export interface MdxJsxTextElementHast extends HastParent {
  type: "mdxJsxTextElement";
  name: string | null;
  attributes: Array<MdxJsxAttribute | MdxJsxExpressionAttribute>;
  children: ElementContent[];
  data?: MdxJsxTextElementHastData | undefined;
}

export interface MdxJsxTextElementHastData extends HastData {}

export interface MdxFlowExpression extends MdastLiteral {
  type: "mdxFlowExpression";
  data?: MdxFlowExpressionData | undefined;
}

export interface MdxFlowExpressionData extends MdastData {}

export interface MdxTextExpression extends MdastLiteral {
  type: "mdxTextExpression";
  data?: MdxTextExpressionData | undefined;
}

export interface MdxTextExpressionData extends MdastData {}

export interface MdxFlowExpressionHast extends HastLiteral {
  type: "mdxFlowExpression";
  data?: MdxFlowExpressionHastData | undefined;
}

export interface MdxFlowExpressionHastData extends HastData {}

export interface MdxTextExpressionHast extends HastLiteral {
  type: "mdxTextExpression";
  data?: MdxTextExpressionHastData | undefined;
}

export interface MdxTextExpressionHastData extends HastData {}

export interface MdxjsEsm extends MdastLiteral {
  type: "mdxjsEsm";
  data?: MdxjsEsmData | undefined;
}

export interface MdxjsEsmData extends MdastData {}

export interface MdxjsEsmHast extends HastLiteral {
  type: "mdxjsEsm";
  data?: MdxjsEsmHastData | undefined;
}

export interface MdxjsEsmHastData extends HastData {}

declare module "mdast" {
  interface BlockContentMap {
    mdxJsxFlowElement: MdxJsxFlowElement;
    mdxFlowExpression: MdxFlowExpression;
  }

  interface PhrasingContentMap {
    mdxJsxTextElement: MdxJsxTextElement;
    mdxTextExpression: MdxTextExpression;
  }

  interface RootContentMap {
    mdxJsxFlowElement: MdxJsxFlowElement;
    mdxJsxTextElement: MdxJsxTextElement;
    mdxTextExpression: MdxTextExpression;
    mdxFlowExpression: MdxFlowExpression;
    mdxjsEsm: MdxjsEsm;
  }

  interface FrontmatterContentMap {
    mdxjsEsm: MdxjsEsm;
  }
}

declare module "hast" {
  interface ElementContentMap {
    mdxJsxTextElement: MdxJsxTextElementHast;
    mdxJsxFlowElement: MdxJsxFlowElementHast;
    mdxFlowExpression: MdxFlowExpressionHast;
    mdxTextExpression: MdxTextExpressionHast;
  }

  interface RootContentMap {
    mdxJsxTextElement: MdxJsxTextElementHast;
    mdxJsxFlowElement: MdxJsxFlowElementHast;
    mdxFlowExpression: MdxFlowExpressionHast;
    mdxTextExpression: MdxTextExpressionHast;
    mdxjsEsm: MdxjsEsmHast;
  }
}
