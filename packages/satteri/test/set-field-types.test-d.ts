import type { Element } from "hast";
import type { Table } from "mdast";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
import type { MdastVisitorContext } from "../src/mdast/mdast-visitor.js";
import type { LeafDirective, MdxJsxFlowElement, MdxJsxFlowElementHast } from "../src/types.js";

declare const hast: HastVisitorContext;
declare const mdast: MdastVisitorContext;
declare const element: Element;
declare const jsx: MdxJsxFlowElementHast;
declare const mdxJsx: MdxJsxFlowElement;
declare const table: Table;
declare const directive: LeafDirective;

hast.setField(element, "tagName", "span");
hast.setField(jsx, "name", null);
mdast.setField(directive, "name", "warning");
mdast.setField(mdxJsx, "attributes", []);
mdast.setField(directive, "attributes", { id: "intro" });
mdast.setField(table, "align", ["right", null]);
hast.setField(element, "properties", {});
hast.setField(jsx, "attributes", []);
// @ts-expect-error the discriminant cannot be changed in place
hast.setField(element, "type", "text");
// @ts-expect-error source positions are node metadata, not settable fields
hast.setField(element, "position", null);

mdast.setAttribute(directive, "id", "intro");
// @ts-expect-error directive attributes only accept strings
mdast.setAttribute(directive, "class", ["a", "b"]);
