// Local interfaces avoid a runtime dependency on mdast-util-directive.

import type {
  BlockContent,
  Data as MdastData,
  DefinitionContent,
  Parent as MdastParent,
  PhrasingContent,
} from "mdast";

// Accept nullish attribute values for compatibility with mdast-util-directive.
export type DirectiveAttributes = Record<string, string | null | undefined>;

export interface ContainerDirective extends MdastParent {
  type: "containerDirective";
  name: string;
  attributes?: DirectiveAttributes | null | undefined;
  children: Array<BlockContent | DefinitionContent>;
  data?: ContainerDirectiveData | undefined;
}

export interface ContainerDirectiveData extends MdastData {}

export interface LeafDirective extends MdastParent {
  type: "leafDirective";
  name: string;
  attributes?: DirectiveAttributes | null | undefined;
  children: PhrasingContent[];
  data?: LeafDirectiveData | undefined;
}

export interface LeafDirectiveData extends MdastData {}

export interface TextDirective extends MdastParent {
  type: "textDirective";
  name: string;
  attributes?: DirectiveAttributes | null | undefined;
  children: PhrasingContent[];
  data?: TextDirectiveData | undefined;
}

export interface TextDirectiveData extends MdastData {}

declare module "mdast" {
  interface BlockContentMap {
    containerDirective: ContainerDirective;
    leafDirective: LeafDirective;
  }

  interface ParagraphData {
    /** Marks a container directive’s label paragraph (`:::note[label]`). */
    directiveLabel?: boolean | null | undefined;
  }

  interface PhrasingContentMap {
    textDirective: TextDirective;
  }

  interface RootContentMap {
    containerDirective: ContainerDirective;
    leafDirective: LeafDirective;
    textDirective: TextDirective;
  }
}
