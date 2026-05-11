import { markdownToHtml, defineMdastPlugin } from "satteri";

// Replace emoji shortcodes in text nodes
const emojis = defineMdastPlugin({
  name: "emojis",
  text(node, ctx) {
    if (node.value.includes(":wave:")) {
      ctx.setProperty(node, "value", node.value.replaceAll(":wave:", "\u{1F44B}"));
    }
  },
});

// Unwrap images from paragraphs (like remark-unwrap-images)
const unwrapImages = defineMdastPlugin({
  name: "unwrap-images",
  paragraph(node) {
    const child = node.children[0];
    if (node.children.length === 1 && child?.type === "image") {
      return child;
    }
  },
});

const source = `
# Hello :wave:

![photo](https://example.com/photo.jpg)

Some text :wave: more text
`;

const { html } = markdownToHtml(source, {
  mdastPlugins: [emojis, unwrapImages],
});

console.log(html);
