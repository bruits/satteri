import type { Comment, Text } from "hast";
import type { Blockquote, Emphasis, Link, ListItem, Paragraph, Strong, TableCell } from "mdast";
import { defineHastPlugin, defineMdastPlugin } from "../src/index.js";
import type { HastVisitorFn, MdastVisitorFn } from "../src/index.js";

// One visitor shared by every node type it is registered for.
type ImageParent = Paragraph | Blockquote | ListItem | Emphasis | Strong | Link | TableCell;

const visitImageParent: MdastVisitorFn<ImageParent> = (node, ctx) => {
  if (node.children.length === 0) ctx.removeNode(node);
};

defineMdastPlugin({
  name: "image-parents",
  paragraph: visitImageParent,
  blockquote: visitImageParent,
  listItem: visitImageParent,
  emphasis: visitImageParent,
  strong: visitImageParent,
  link: visitImageParent,
  tableCell: visitImageParent,
});

// @ts-expect-error - a heading is not one of the node types the visitor accepts
defineMdastPlugin({ name: "headings", heading: visitImageParent });

const visitTextOrComment: HastVisitorFn<Text | Comment> = (node) => {
  if (node.value === "") return { type: "text", value: " " };
};

defineHastPlugin({
  name: "text-and-comments",
  text: visitTextOrComment,
  comment: visitTextOrComment,
});

// @ts-expect-error - a doctype is not one of the node types the visitor accepts
defineHastPlugin({ name: "doctypes", doctype: visitTextOrComment });
