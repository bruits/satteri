// Manually-defined directive AST node types.
//
// These replicate the type definitions from mdast-util-directive so we can
// avoid pulling in that package (and its transitive deps) just for the
// node interfaces. The shapes match what `getDirectiveData` returns from
// the Rust arena: `name: string`, `attributes: Record<string, string>`.

import type {
  BlockContent,
  Data as MdastData,
  DefinitionContent,
  Parent as MdastParent,
  PhrasingContent,
} from "mdast";

// Even though `null` and `undefined` values are omitted in both Sätteri and mdast-util-directive,
// they're allowed in the type definitions here to match the mdast-util-directive type.
// https://github.com/syntax-tree/mdast-util-directive/blob/a683327fafc4e48f81caf8d09d15fef8dd42a627/lib/index.js#L212-L213
// https://github.com/syntax-tree/mdast-util-directive/blob/main/index.d.ts#L49
export type DirectiveAttributes = Record<string, string | null | undefined>;

export interface ContainerDirective extends MdastParent {
  type: "containerDirective";
  name: string;
  attributes?: DirectiveAttributes | null | undefined;
  children: Array<BlockContent | DefinitionContent>;
  data?: ContainerDirectiveData | undefined;
}

export interface ContainerDirectiveData extends MdastData {
  directiveLabel?: boolean | undefined;
}

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

  interface PhrasingContentMap {
    textDirective: TextDirective;
  }

  interface RootContentMap {
    containerDirective: ContainerDirective;
    leafDirective: LeafDirective;
    textDirective: TextDirective;
  }
}
