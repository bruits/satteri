import { describe, test, expect } from "vitest";
import MarkdownIt from "markdown-it";
import markdownItAttrs from "markdown-it-attrs";
import { markdownToHtml } from "../../src/index.js";

// markdown-it-attrs is the reference: remark has no maintained attributes plugin.
const reference = new MarkdownIt().use(markdownItAttrs);

// Line breaks between block tags differ between the renderers, whatever the attributes.
function collapseTagWhitespace(html: string): string {
  return html.trim().replace(/>\s+</g, "><");
}

// markdown-it renders strikethrough as `<s>` where remark and Sätteri use `<del>`.
function referenceHtml(md: string): string {
  return collapseTagWhitespace(reference.render(md)).replace(/<(\/?)s([ >])/g, "<$1del$2");
}

function satteriHtml(md: string): string {
  const result = markdownToHtml(md, { features: { attributes: true } });
  if (result instanceof Promise) throw new Error("expected sync");
  return collapseTagWhitespace(result.html);
}

describe("attributes: conformance vs markdown-it-attrs", () => {
  const CASES: Array<[string, string]> = [
    ["heading attributes in source order", "# Heading {.title #top}"],
    ["heading with a key-value and merged classes", "## Section {#s data-x=1 .a .b}"],
    ["fence: attribute class before the language class", "```js {.highlight}\nx\n```"],
    ["fence: class, id and key-value", "```ts {.foo #bar data-lang=typescript}\nx\n```"],
    ["fence: no language", "``` {.plain}\nx\n```"],
    ["fence: no space before the block", "```js{.a}\nx\n```"],
    ["fence: tilde fence", "~~~js {.a}\nx\n~~~"],
    ["fence: text before the block stays meta", '```js title="x" {.a}\nx\n```'],
    ["fence: only the last block is read", "```js {.a} {.b}\nx\n```"],
    ["fence: a block that doesn't end the info string", "```js {.a} extra\nx\n```"],
    ["fence: an empty block", "```js {}\nx\n```"],
    ["fence: an unclosed block", "```js {.a\nx\n```"],
    ["inline: emphasis, strong and a code span", "*em*{.a} **strong**{#b} `code`{c=d}"],
    [
      "inline: link, image and reference link",
      "[link](/u){.a} ![alt](/i.png){.b} [ref][r]{#c}\n\n[r]: /r",
    ],
    ["inline: strikethrough", "~~gone~~{.x}"],
    ["inline: nested elements", "**a *b*{.in}**{.out}"],
    ["inline: blocks that follow one another merge", "*em*{.a}{#i}{.b k=v}"],
    ["inline: markup characters inside a block", "*em*{.a_b}"],
    ["inline: a block after a space or plain text stays text", "*em* {.a} text{.b} more"],
    ["inline: an escaped, empty or unclosed block stays text", "*em*\\{.c} *em*{} *em*{.d"],
    ["block: ending a paragraph's last line", "A paragraph. {.intro}"],
    ["block: a paragraph's last line of its own", "Another\n{#second}"],
    ["block: a line of its own after a hard break", "line\\\n{.x}"],
    ["block: a list item's first line", "- item {.red}\n- item"],
    ["block: a line of its own in a list item goes on the list", "- a\n- b\n{.my-list}"],
    ["block: in a nested list item, the nested list", "- parent\n  - child\n  {.inner}"],
    ["block: a line of its own in a blockquote", "> quoted\n> {.note}"],
    ["block: a paragraph of its own after a list", "- a\n- b\n\n{.my-list}"],
    [
      "block: table cells and a paragraph of its own after the table",
      "| Head {.h} |\n| --- |\n| cell {.c} |\n\n{.my-table}",
    ],
    ["block: a paragraph of its own anywhere else stays, empty", "# H\n\n{.alone}"],
    ["block: only the last of several blocks ending a paragraph", "text{.a}{.b}"],
    ["block: a block mid-paragraph stays text", "para\n{.x}\nmore"],
    ["block: braces in image alt text", "![{.cls}](x.png)"],
    ["block: an inline element before the block takes it", "![alt](img.png){.img-class}"],
  ];

  for (const [name, md] of CASES) {
    test(name, () => {
      expect(satteriHtml(md)).toBe(referenceHtml(md));
    });
  }

  // The language is written before the braces, so its class comes first, in
  // source order; markdown-it-attrs appends it after the attributes instead.
  test("fence: the language class comes first, unlike the reference", () => {
    const md = "```python {#code-block}\nx\n```";
    expect(satteriHtml(md)).toBe(
      '<pre><code class="language-python" id="code-block">x\n</code></pre>',
    );
    expect(referenceHtml(md)).toBe(
      '<pre><code id="code-block" class="language-python">x\n</code></pre>',
    );
  });

  // A paragraph of attributes alone goes on a thematic break before it, as on a
  // list or table; markdown-it-attrs keeps it as an empty paragraph there.
  test("block: a paragraph of its own after a thematic break, unlike the reference", () => {
    const md = "---\n{.divider}";
    expect(satteriHtml(md)).toBe('<hr class="divider">');
    expect(referenceHtml(md)).toBe('<hr><p class="divider"></p>');
  });

  // `--- {.x}` isn't a thematic break in CommonMark, so it's a paragraph with
  // attributes; markdown-it-attrs reads it as a thematic break.
  test("block: `--- {.x}` is a paragraph, unlike the reference", () => {
    const md = "--- {.divider}";
    expect(satteriHtml(md)).toBe('<p class="divider">---</p>');
    expect(referenceHtml(md)).toBe('<hr class="divider">');
  });
});
