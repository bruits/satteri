import type { Element } from "hast";
import type { Table } from "mdast";
import type { HastVisitorContext } from "../src/hast/hast-visitor.js";
import type { MdastVisitorContext } from "../src/mdast/mdast-visitor.js";
import type { LeafDirective, MdxJsxFlowElementHast } from "../src/types.js";

declare const hast: HastVisitorContext;
declare const mdast: MdastVisitorContext;
declare const element: Element;
declare const jsx: MdxJsxFlowElementHast;
declare const table: Table;
declare const directive: LeafDirective;

hast.setField(element, "tagName", "span");
hast.setField(jsx, "name", null);
mdast.setField(directive, "name", "warning");

// Container-shaped fields use dedicated mutation methods or a full replacement.
// @ts-expect-error table align is not representable by the scalar setField command
mdast.setField(table, "align", ["right", null]);
// @ts-expect-error HAST properties entries must be changed with setProperty
hast.setField(element, "properties", {});
// @ts-expect-error MDX JSX attributes entries must be changed with setAttribute
hast.setField(jsx, "attributes", []);
// @ts-expect-error the discriminant cannot be changed in place
hast.setField(element, "type", "text");

mdast.setAttribute(directive, "id", "intro");
// @ts-expect-error directive attributes only accept strings
mdast.setAttribute(directive, "class", ["a", "b"]);
