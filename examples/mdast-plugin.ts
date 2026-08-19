import { markdownToHtml, defineMdastPlugin } from "satteri";

const emojis = defineMdastPlugin({
  name: "emojis",
  text(node, ctx) {
    if (node.value.includes(":wave:")) {
      ctx.setField(node, "value", node.value.replaceAll(":wave:", "\u{1F44B}"));
    }
  },
});

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
