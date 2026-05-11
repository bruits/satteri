import { describe, expect, test } from "vitest";
import type { Plugin } from "vite";
import vitePluginSatteri, { satteri, type VitePluginSatteriOptions } from "../src/index.js";

type Command = "serve" | "build";

function makePlugin(options?: VitePluginSatteriOptions, command: Command = "build"): Plugin {
  const plugin = vitePluginSatteri(options);
  const cr = plugin.configResolved;
  const fakeConfig = { command } as unknown as Parameters<
    Extract<typeof cr, (...args: never[]) => unknown>
  >[0];
  if (typeof cr === "function") {
    (cr as (config: typeof fakeConfig) => void).call(undefined, fakeConfig);
  } else if (cr && typeof cr === "object" && "handler" in cr) {
    (cr.handler as (config: typeof fakeConfig) => void).call(undefined, fakeConfig);
  }
  return plugin;
}

async function runTransform(
  plugin: Plugin,
  source: string,
  id: string,
): Promise<{ code: string; map: null } | null> {
  const t = plugin.transform;
  let fn: ((src: string, id: string) => unknown) | undefined;
  if (typeof t === "function") {
    fn = t as unknown as (src: string, id: string) => unknown;
  } else if (t && typeof t === "object" && "handler" in t) {
    fn = t.handler as unknown as (src: string, id: string) => unknown;
  }
  if (!fn) return null;
  const result = await fn.call(undefined, source, id);
  return result as { code: string; map: null } | null;
}

async function compile(plugin: Plugin, source: string, id: string): Promise<string> {
  const result = await runTransform(plugin, source, id);
  if (!result) throw new Error(`expected transform output for ${id}`);
  return result.code;
}

describe("vite-plugin-satteri", () => {
  test("named `satteri` export aliases the default", () => {
    expect(satteri).toBe(vitePluginSatteri);
  });

  test("returns null for non-md/mdx files", async () => {
    const plugin = makePlugin();
    const result = await runTransform(plugin, "const x = 1;", "/src/foo.ts");
    expect(result).toBeNull();
  });

  describe("markdown", () => {
    test("compiles .md to a JS module exporting HTML", async () => {
      const plugin = makePlugin();
      const code = await compile(plugin, "# Hello", "/src/post.md");
      expect(code).toContain("<h1>Hello</h1>");
      expect(code).toContain("export default html");
      expect(code).toContain("export { html }");
    });

    test("escapes backticks and `${}` safely via JSON.stringify", async () => {
      const plugin = makePlugin();
      const code = await compile(plugin, "use `code` and ${expr}", "/src/x.md");
      // Output is a quoted JSON string, not a template literal
      expect(code).toMatch(/const html = "/);
      expect(code).not.toContain("`<p>");
    });

    test("strips query string from id when computing filename", async () => {
      const plugin = makePlugin();
      const result = await runTransform(plugin, "# Hello", "/src/x.md?import");
      expect(result).not.toBeNull();
    });

    test("`markdown: false` skips .md files", async () => {
      const plugin = makePlugin({ markdown: false });
      const result = await runTransform(plugin, "# Hello", "/src/x.md");
      expect(result).toBeNull();
    });

    test("features propagate (gfm: false disables strikethrough)", async () => {
      const plugin = makePlugin({ features: { gfm: false } });
      const code = await compile(plugin, "~~strike~~", "/src/x.md");
      expect(code).not.toContain("<del>");
    });

    test("mdastPlugins receive nodes and can mutate text", async () => {
      const plugin = makePlugin({
        mdastPlugins: [
          {
            name: "swap-foo",
            text(node, ctx) {
              if (node.value.includes("foo")) {
                ctx.setProperty(node, "value", node.value.replaceAll("foo", "bar"));
              }
            },
          },
        ],
      });
      const code = await compile(plugin, "this foo here", "/src/x.md");
      expect(code).toContain("this bar here");
    });

    test("emits an empty frontmatter export when there is none", async () => {
      const plugin = makePlugin();
      const code = await compile(plugin, "# Hello", "/src/x.md");
      expect(code).toContain("export const frontmatter = {};");
    });

    test("parses YAML frontmatter into the frontmatter export", async () => {
      const plugin = makePlugin();
      const source = `---
title: Hello world
draft: false
tags:
  - one
  - two
---

# Body`;
      const code = await compile(plugin, source, "/src/x.md");
      expect(code).toContain(
        'export const frontmatter = {"title":"Hello world","draft":false,"tags":["one","two"]};',
      );
      expect(code).toContain("<h1>Body</h1>");
    });

    test("frontmatter export is empty when frontmatter feature is disabled", async () => {
      const plugin = makePlugin({ features: { frontmatter: false } });
      const source = "---\ntitle: Hello\n---\n# Body";
      const code = await compile(plugin, source, "/src/x.md");
      expect(code).toContain("export const frontmatter = {};");
    });

    test("hastPlugins can mutate elements", async () => {
      const plugin = makePlugin({
        hastPlugins: [
          {
            name: "tag-headings",
            element: {
              filter: ["h1"],
              visit(node, ctx) {
                ctx.setProperty(node, "className", ["heading"]);
              },
            },
          },
        ],
      });
      const code = await compile(plugin, "# Hello", "/src/x.md");
      expect(code).toContain('class=\\"heading\\"');
    });
  });

  describe("mdx", () => {
    test("compiles .mdx to a JS module with MDXContent", async () => {
      const plugin = makePlugin();
      const code = await compile(plugin, "# Hello", "/src/post.mdx");
      expect(code).toContain("MDXContent");
      expect(code).toMatch(/react\/jsx[\w-]*runtime/);
    });

    test("`mdx: false` skips .mdx files", async () => {
      const plugin = makePlugin({ mdx: false });
      const result = await runTransform(plugin, "# Hello", "/src/x.mdx");
      expect(result).toBeNull();
    });

    test("`mdx: true` is equivalent to default options", async () => {
      const plugin = makePlugin({ mdx: true });
      const code = await compile(plugin, "# Hello", "/src/x.mdx");
      expect(code).toContain("MDXContent");
    });

    test("mdx options object propagates jsxImportSource", async () => {
      const plugin = makePlugin({ mdx: { jsxImportSource: "preact" } });
      const code = await compile(plugin, "# Hello", "/src/x.mdx");
      expect(code).toContain('"preact/jsx-runtime"');
      expect(code).not.toContain('"react/jsx-runtime"');
    });

    test("infers `development: true` from Vite serve command", async () => {
      const plugin = makePlugin(undefined, "serve");
      const code = await compile(plugin, "# Hello", "/src/x.mdx");
      expect(code).toContain("jsx-dev-runtime");
    });

    test("infers `development: false` from Vite build command", async () => {
      const plugin = makePlugin(undefined, "build");
      const code = await compile(plugin, "# Hello", "/src/x.mdx");
      expect(code).not.toContain("jsx-dev-runtime");
    });

    test("explicit `development` overrides the inferred value", async () => {
      const plugin = makePlugin({ mdx: { development: false } }, "serve");
      const code = await compile(plugin, "# Hello", "/src/x.mdx");
      expect(code).not.toContain("jsx-dev-runtime");
    });

    test("optimizeStatic collapses static subtrees", async () => {
      const plugin = makePlugin({
        mdx: {
          optimizeStatic: { component: "Fragment", prop: "set:html" },
        },
      });
      const code = await compile(plugin, "# Hello\n\nA paragraph.", "/src/x.mdx");
      expect(code).toContain("set:html");
    });

    test("YAML frontmatter is exported alongside the MDX module", async () => {
      const plugin = makePlugin();
      const source = `---
title: MDX Hello
---

# Body`;
      const code = await compile(plugin, source, "/src/x.mdx");
      expect(code).toContain('export const frontmatter = {"title":"MDX Hello"};');
      expect(code).toContain("MDXContent");
    });

    test("shared mdastPlugins also run for .mdx", async () => {
      const plugin = makePlugin({
        mdastPlugins: [
          {
            name: "swap-foo",
            text(node, ctx) {
              if (node.value.includes("foo")) {
                ctx.setProperty(node, "value", node.value.replaceAll("foo", "bar"));
              }
            },
          },
        ],
      });
      const code = await compile(plugin, "this foo here", "/src/x.mdx");
      expect(code).toContain("bar");
      expect(code).not.toContain('"foo"');
    });
  });
});
