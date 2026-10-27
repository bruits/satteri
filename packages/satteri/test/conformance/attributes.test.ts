import { describe, test, expect } from "vitest";
import MarkdownIt from "markdown-it";
import markdownItAttrs from "markdown-it-attrs";
import { markdownToHtml } from "../../src/index.js";

// markdown-it-attrs is the reference: remark has no maintained attributes plugin.
// Its default moves fence attributes to `<pre>`; Sätteri keeps them on `<code>`, as mdast-util-to-hast does.
const reference = new MarkdownIt().use(markdownItAttrs, { fenceAttrsOnPre: false });

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
    [
      "fence: a brace inside a quoted value",
      '``` {title="a}b"}\nx\n```\n\n``` {title="a{b"}\nx\n```',
    ],
    ["fence: braces written as character references", "```js &#123;.a&#125;\nx\n```"],
    ["inline: emphasis, strong and a code span", "*em*{.a} **strong**{#b} `code`{c=d}"],
    [
      "inline: link, image and reference link",
      "[link](/u){.a} ![alt](/i.png){.b} [ref][r]{#c}\n\n[r]: /r",
    ],
    ["inline: strikethrough", "~~gone~~{.x}"],
    ["inline: nested elements", "**a *b*{.in}**{.out}"],
    [
      "inline: an element ending a heading takes the block",
      "# Title **b**{.x}\n\n## See [the guide](/g){.y}",
    ],
    ["inline: blocks that follow one another merge", "*em*{.a}{#i}{.b k=v}"],
    ["inline: markup characters inside a block", "*em*{.a_b}"],
    ["inline: a block after a space or plain text stays text", "*em* {.a} text{.b} more"],
    ["inline: an escaped, empty or unclosed block stays text", "*em*\\{.c} *em*{} *em*{.d"],
    ["block: ending a paragraph's last line", "A paragraph. {.intro}"],
    ["block: a paragraph's last line of its own", "Another\n{#second}"],
    ["block: a line of its own after a hard break", "line\\\n{.x}"],
    ["block: a list item's first line", "- item {.red}\n- item"],
    ["block: a lazy line of its own after a list item goes on the list", "- a\n- b\n{.my-list}"],
    ["block: in a nested list item, the nested list", "- parent\n  - child\n  {.inner}"],
    [
      "block: nested lists take blocks by indentation",
      "- item 1{.a}\n  - nested item {.b}\n  {.c}\n    1. nested nested item {.d}\n    {.e}",
    ],
    [
      "block: a paragraph of its own goes on the outer list",
      "- item\n  - nested\n  {.red}\n\n{.blue}",
    ],
    ["block: a lazy line of its own after a blockquote goes on it", "> quote\n{.c}"],
    [
      "block: a line of its own before more blocks goes on the paragraph",
      "> para\n> {.x}\n>\n> more",
    ],
    ["block: a paragraph of its own after a list", "- a\n- b\n\n{.my-list}"],
    [
      "block: table cells and a paragraph of its own after the table",
      "| Head {.h} |\n| --- |\n| cell {.c} |\n\n{.my-table}",
    ],
    ["block: a paragraph of its own anywhere else stays, empty", "# H\n\n{.alone}"],
    ["block: only the last of several blocks ending a paragraph", "text{.a}{.b}"],
    [
      "block: a brace inside a quoted value",
      'para {title="a{b"}\n\n| h {title="a}b"} |\n| --- |\n| c {title="{"} |',
    ],
    ["block: a block mid-paragraph stays text", "para\n{.x}\nmore"],
    ["block: braces in image alt text", "![{.cls}](x.png)"],
    ["block: an inline element before the block takes it", "![alt](img.png){.img-class}"],
    ["block: after markup left as text", "see [brackets]{.a}\n\nfoo_{.b}\n\n# see [brackets]{.c}"],
    ["inline: collapsed references", "[ref][]{.a} ![ref][]{.b}\n\n[ref]: /u"],
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

  // A line of its own goes on the block it's written after, at the level its `>`
  // markers and indentation put it, as kramdown places a block IAL. markdown-it
  // drops a line's indentation before plugins see it, so markdown-it-attrs gives
  // the line to the outermost block that ends with the paragraph; its author
  // describes the indentation rule as the one they wanted (markdown-it-attrs#32).
  const OWN_LINE: Array<[string, string, string, string]> = [
    [
      "in a blockquote, the paragraph",
      "> quoted\n> {.note}",
      '<blockquote><p class="note">quoted</p></blockquote>',
      '<blockquote class="note"><p>quoted</p></blockquote>',
    ],
    [
      "in the outer of two blockquotes, the inner",
      "> > nested\n> {.inner}",
      '<blockquote><blockquote class="inner"><p>nested</p></blockquote></blockquote>',
      '<blockquote class="inner"><blockquote><p>nested</p></blockquote></blockquote>',
    ],
    [
      "in a tight list item, the item",
      "- a\n  {.x}",
      '<ul><li class="x">a</li></ul>',
      '<ul class="x"><li>a</li></ul>',
    ],
    [
      "in a loose list item, the paragraph",
      "- a\n\n  para\n  {.x}",
      '<ul><li><p>a</p><p class="x">para</p></li></ul>',
      '<ul class="x"><li><p>a</p><p>para</p></li></ul>',
    ],
    [
      "in a blockquote in a list item, the paragraph",
      "- a\n  > q\n  > {.x}",
      '<ul><li>a\n<blockquote><p class="x">q</p></blockquote></li></ul>',
      '<ul class="x"><li>a\n<blockquote><p>q</p></blockquote></li></ul>',
    ],
  ];

  for (const [name, md, ours, theirs] of OWN_LINE) {
    test(`block: a line of its own ${name}, unlike the reference`, () => {
      expect(satteriHtml(md)).toBe(ours);
      expect(referenceHtml(md)).toBe(theirs);
    });
  }

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
