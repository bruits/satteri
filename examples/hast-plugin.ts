import { markdownToHtml, defineHastPlugin } from "satteri";

// Add target="_blank" and rel="noopener" to external links
const externalLinks = defineHastPlugin({
  name: "external-links",
  element: {
    filter: ["a"],
    visit(node, ctx) {
      const href = node.properties.href;
      if (typeof href === "string" && href.startsWith("http")) {
        ctx.setProperty(node, "target", "_blank");
        ctx.setProperty(node, "rel", "noopener noreferrer");
      }
    },
  },
});

// Add IDs to headings
const headingIds = defineHastPlugin({
  name: "heading-ids",
  element: {
    filter: ["h1", "h2", "h3", "h4", "h5", "h6"],
    visit(node, ctx) {
      const text = ctx.textContent(node);
      const id = text
        .toLowerCase()
        .replace(/\s+/g, "-")
        .replace(/[^\w-]/g, "");
      ctx.setProperty(node, "id", id);
    },
  },
});

const source = `
# Getting Started

Check out [the docs](https://example.com/docs) for more info.

## Installation

Run the install command and [follow the guide](/guide).
`;

const { html } = markdownToHtml(source, {
  hastPlugins: [externalLinks, headingIds],
});

console.log(html);
