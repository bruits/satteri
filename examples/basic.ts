import { markdownToHtml, mdxToJs } from "satteri";

// Markdown to HTML
const { html } = markdownToHtml("# Hello\n\n**Bold** and *italic* text.");
console.log(html);

// MDX to JavaScript
const { code } = mdxToJs("# Hello\n\n<MyComponent foo='bar' />");
console.log(code);
