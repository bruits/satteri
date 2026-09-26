import { markdownToHtml, mdxToJs } from "satteri";

const { html } = markdownToHtml("# Hello\n\n**Bold** and *italic* text.");
console.log(html);

const { code } = mdxToJs("# Hello\n\n<MyComponent foo='bar' />");
console.log(code);
