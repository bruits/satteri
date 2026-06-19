import { describe, test, expect } from "vitest";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkDirective from "remark-directive";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";
import { h as _h, s as _s, type Properties } from "hastscript";
import type { Root as MdastRoot, Paragraph as P, PhrasingContent } from "mdast";
import { markdownToHtml, defineMdastPlugin } from "../../src/index.js";
import type { MdastNode } from "../../src/types.js";

type Variant = "note" | "tip" | "caution" | "danger";
const variants: Variant[] = ["note", "tip", "caution", "danger"];
const variantSet = new Set<string>(variants);
const isAsideVariant = (s: string): s is Variant => variantSet.has(s);

const defaultTitles: Record<Variant, string> = {
  note: "Note",
  tip: "Tip",
  caution: "Caution",
  danger: "Danger",
};

function h(el: string, attrs: Properties = {}, children: unknown[] = []): P {
  const { tagName, properties } = _h(el, attrs);
  return {
    type: "paragraph",
    data: { hName: tagName, hProperties: properties },
    children: children as P["children"],
  };
}

function s(el: string, attrs: Properties = {}, children: unknown[] = []): P {
  const { tagName, properties } = _s(el, attrs);
  return {
    type: "paragraph",
    data: { hName: tagName, hProperties: properties },
    children: children as P["children"],
  };
}

const iconPaths: Record<Variant, P[]> = {
  note: [s("path", { d: "M12 2L2 22h20L12 2z" })],
  tip: [s("path", { d: "M5 5l7 14 7-14H5z" })],
  caution: [s("path", { d: "M1 21h22L12 2 1 21z" })],
  danger: [s("path", { d: "M0 0h24v24H0z" })],
};

function buildAside(
  variant: Variant,
  title: string,
  titleNode: PhrasingContent[],
  body: P["children"],
): P {
  return h(
    "aside",
    {
      "aria-label": title,
      class: `starlight-aside starlight-aside--${variant}`,
    },
    [
      h("p", { class: "starlight-aside__title", "aria-hidden": "true" }, [
        s(
          "svg",
          {
            viewBox: "0 0 24 24",
            width: 16,
            height: 16,
            fill: "currentColor",
            class: "starlight-aside__icon",
          },
          iconPaths[variant],
        ),
        ...titleNode,
      ]),
      h("div", { class: "starlight-aside__content" }, body),
    ],
  );
}

// Matches `mdast-util-to-string` (and satteri's `ctx.textContent`) so the two
// pipelines agree on `inlineCode` etc. when computing the aside's aria-label.
function nodeText(node: { type: string; value?: string; children?: { type: string }[] }): string {
  if (typeof node.value === "string") return node.value;
  if (Array.isArray(node.children)) {
    return (node.children as Parameters<typeof nodeText>[0][]).map(nodeText).join("");
  }
  return "";
}

function transformAsides(tree: MdastRoot): void {
  const walk = (parent: { children?: unknown[] }): void => {
    if (!Array.isArray(parent.children)) return;
    const kids = parent.children as Array<{
      type: string;
      name?: string;
      children?: unknown[];
      data?: Record<string, unknown>;
    }>;
    for (let i = 0; i < kids.length; i++) {
      const node = kids[i];
      if (!node) continue;
      if (node.type === "containerDirective" && node.name && isAsideVariant(node.name)) {
        const variant = node.name;
        let title = defaultTitles[variant];
        let titleNode: PhrasingContent[] = [{ type: "text", value: title }];
        const children = [...((node.children ?? []) as P[])];
        const firstChild = children[0];
        if (
          firstChild?.type === "paragraph" &&
          firstChild.data?.directiveLabel &&
          firstChild.children.length > 0
        ) {
          titleNode = firstChild.children;
          title = nodeText(firstChild as Parameters<typeof nodeText>[0]);
          children.shift();
        }
        // Recurse into the body first so a nested directive becomes an aside too
        // (innermost-first), matching how the real remarkAsides plugin composes.
        walk({ children });
        kids[i] = buildAside(
          variant,
          title,
          titleNode,
          children as unknown as P["children"],
        ) as unknown as (typeof kids)[number];
      } else {
        walk(node as { children?: unknown[] });
      }
    }
  };
  walk(tree);
}

function asidesPluginSatteri() {
  return defineMdastPlugin({
    name: "asides",
    containerDirective(node, ctx) {
      if (!isAsideVariant(node.name)) return;
      const variant = node.name;
      let title = defaultTitles[variant];
      let titleNode: PhrasingContent[] = [{ type: "text", value: title }];
      const children = [...node.children] as P[];
      const firstChild = children[0];
      if (
        firstChild?.type === "paragraph" &&
        firstChild.data?.directiveLabel &&
        firstChild.children.length > 0
      ) {
        titleNode = firstChild.children;
        title = ctx.textContent(firstChild);
        children.shift();
      }
      return buildAside(
        variant,
        title,
        titleNode,
        children as unknown as P["children"],
      ) as unknown as MdastNode;
    },
  });
}

function referenceHtml(md: string): string {
  const processor = unified()
    .use(remarkParse)
    .use(remarkDirective)
    .use(() => transformAsides)
    .use(remarkRehype, { allowDangerousHtml: true })
    .use(rehypeStringify, { allowDangerousHtml: true });
  return normalize(String(processor.processSync(md)));
}

function satteriHtml(md: string): string {
  const { html } = markdownToHtml(md, {
    features: { directive: true, gfm: false, frontmatter: false, math: false },
    mdastPlugins: [asidesPluginSatteri()],
  });
  return normalize(html);
}

function normalize(html: string): string {
  return html.trim();
}

describe("Starlight asides plugin (fresh-node data hints, full integration)", () => {
  for (const variant of variants) {
    test(`:::${variant} (default title) renders identical HTML`, () => {
      const md = `:::${variant}\nBody text.\n:::`;
      expect(satteriHtml(md)).toBe(referenceHtml(md));
    });

    test(`:::${variant}[Custom Title] renders identical HTML`, () => {
      const md = `:::${variant}[Heads up!]\nBody text.\n:::`;
      expect(satteriHtml(md)).toBe(referenceHtml(md));
    });
  }

  test("aside body preserves multiple paragraphs and inline formatting", () => {
    const md = `:::tip[Did you know?]\nFirst paragraph with **bold** and *italic*.\n\nSecond paragraph with [a link](https://example.com).\n:::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });

  test("aside label preserves inline `code`", () => {
    const md = `:::note[See \`config\`]\nDetails.\n:::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });

  // Issue 3: emphasis/strong inside a directive label (not just inline code).
  test("aside label parses emphasis and strong", () => {
    const md = `:::note[Custom **strong with _emphasis_** Label]\nSome text\n:::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });

  test("aside label parses a link", () => {
    const md = `:::tip[See [the docs](https://example.com)]\nBody.\n:::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });

  // Issue 4: a container directive whose body ends with an HTML block must still
  // close on the `:::` fence rather than swallowing it into the HTML block.
  test("aside body ending in an HTML block closes cleanly", () => {
    const md = `:::note\nParagraph.\n\n<details>\n<summary>See more</summary>\n\nMore.\n\n</details>\n:::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });

  // Issue 1: a nested directive inside a transformed directive must transform too.
  test("nested asides", () => {
    const md = `::::note\nNote contents.\n\n:::tip\nNested tip.\n:::\n\n::::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });

  test("nested asides with custom titles", () => {
    const md = `:::::caution[Caution with a custom title]\nNested caution.\n\n::::note\nNested note.\n\n:::tip[Tip with a custom title]\nNested tip.\n:::\n\n::::\n\n:::::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });

  test("triply nested asides", () => {
    const md = `:::::note\nA\n\n::::tip\nB\n\n:::caution\nC\n:::\n\n::::\n\n:::::`;
    expect(satteriHtml(md)).toBe(referenceHtml(md));
  });
});
