import { describe, test, expect } from "vitest";
import { markdownToHtml, markdownToJs, mdxToJs, defineMdastPlugin } from "../src/index.js";
import type { PluginFactoryContext } from "../src/index.js";

/** Records its name on each heading, making run order observable. */
function recordMdast(order: string[], name: string) {
  return defineMdastPlugin({
    name,
    heading() {
      order.push(name);
    },
  });
}

describe("plugin factory context", () => {
  test("a factory receives the source, fileURL, sourceFormat and data", () => {
    const seen: PluginFactoryContext[] = [];
    const data = { seeded: true };
    const fileURL = new URL("file:///docs/something.md");

    markdownToHtml("# Title", {
      fileURL,
      data,
      mdastPlugins: [
        (ctx) => {
          seen.push(ctx);
          return null;
        },
      ],
    });

    expect(seen).toHaveLength(1);
    expect(seen[0]!.source).toBe("# Title");
    expect(seen[0]!.fileURL).toBe(fileURL);
    expect(seen[0]!.sourceFormat).toBe("markdown");
    expect(seen[0]!.data).toBe(data);
  });

  test("fileURL is undefined when the option was not given", () => {
    let seen: URL | undefined | "unset" = "unset";

    markdownToHtml("# Title", {
      mdastPlugins: [
        (ctx) => {
          seen = ctx.fileURL;
          return null;
        },
      ],
    });

    expect(seen).toBeUndefined();
  });

  test("sourceFormat distinguishes mdx from markdown", () => {
    const formats: string[] = [];
    const probe = (ctx: PluginFactoryContext) => {
      formats.push(ctx.sourceFormat);
      return null;
    };

    markdownToHtml("# Title", { mdastPlugins: [probe] });
    markdownToJs("# Title", { mdastPlugins: [probe] });
    mdxToJs("# Title", { mdastPlugins: [probe] });

    expect(formats).toEqual(["markdown", "markdown", "mdx"]);
  });

  // markdownToJs runs the MDX pipeline with a different parser, so it could plausibly report "mdx".
  test("hast factories see the entry point's sourceFormat", () => {
    const formats: string[] = [];
    const probe = (ctx: PluginFactoryContext) => {
      formats.push(ctx.sourceFormat);
      return null;
    };

    markdownToHtml("# Title", { hastPlugins: [probe] });
    markdownToJs("# Title", { hastPlugins: [probe] });
    mdxToJs("# Title", { hastPlugins: [probe] });

    expect(formats).toEqual(["markdown", "markdown", "mdx"]);
  });

  test("hast factories receive the same context", () => {
    const seen: PluginFactoryContext[] = [];
    const fileURL = new URL("file:///docs/page.mdx");

    mdxToJs("# Title", {
      fileURL,
      hastPlugins: [
        (ctx) => {
          seen.push(ctx);
          return null;
        },
      ],
    });

    expect(seen).toHaveLength(1);
    expect(seen[0]!.sourceFormat).toBe("mdx");
    expect(seen[0]!.fileURL).toBe(fileURL);
  });

  test("a plugin runs only for the file it opts into", () => {
    const order: string[] = [];
    const onlySomething = (ctx: PluginFactoryContext) =>
      ctx.fileURL?.pathname.endsWith("/something.md") ? recordMdast(order, "gated") : null;

    markdownToHtml("# Title", {
      fileURL: new URL("file:///docs/other.md"),
      mdastPlugins: [onlySomething, recordMdast(order, "always")],
    });
    markdownToHtml("# Title", {
      fileURL: new URL("file:///docs/something.md"),
      mdastPlugins: [onlySomething, recordMdast(order, "always")],
    });

    expect(order).toEqual(["always", "gated", "always"]);
  });

  test("a preset factory gates its whole bundle per document", () => {
    const order: string[] = [];
    const preset = (ctx: PluginFactoryContext) =>
      ctx.sourceFormat === "mdx" ? [recordMdast(order, "a"), recordMdast(order, "b")] : null;

    markdownToHtml("# Title", { mdastPlugins: [preset] });
    expect(order).toEqual([]);

    mdxToJs("# Title", { mdastPlugins: [preset] });
    expect(order).toEqual(["a", "b"]);
  });

  test("factories nested inside bundles receive the context too", () => {
    const seen: string[] = [];

    mdxToJs("# Title", {
      mdastPlugins: [
        [
          (ctx) => {
            seen.push(ctx.sourceFormat);
            return null;
          },
          [
            (ctx) => {
              seen.push(ctx.source);
              return null;
            },
          ],
        ],
      ],
    });

    expect(seen).toEqual(["mdx", "# Title"]);
  });

  test("a factory returned by a factory also receives the context", () => {
    const seen: string[] = [];

    mdxToJs("# Title", {
      mdastPlugins: [
        () => (ctx: PluginFactoryContext) => {
          seen.push(ctx.sourceFormat);
          return null;
        },
      ],
    });

    expect(seen).toEqual(["mdx"]);
  });

  test("the data bag a factory sees is the one visitors mutate", () => {
    const data: Record<string, unknown> = {};

    const result = markdownToHtml("# Title", {
      data,
      mdastPlugins: [
        (ctx) => {
          ctx.data.fromFactory = true;
          return defineMdastPlugin({
            name: "reader",
            heading(_node, visitorCtx) {
              visitorCtx.data.sawFactoryValue = visitorCtx.data.fromFactory === true;
            },
          });
        },
      ],
    });

    expect(result.data.fromFactory).toBe(true);
    expect(result.data.sawFactoryValue).toBe(true);
  });

  test("a zero-argument factory keeps working", () => {
    const order: string[] = [];

    markdownToHtml("# Title", { mdastPlugins: [() => recordMdast(order, "legacy")] });

    expect(order).toEqual(["legacy"]);
  });

  test("gating every plugin off leaves the document untouched", () => {
    const result = markdownToHtml("# Title\n\nText.", {
      mdastPlugins: [() => null],
      hastPlugins: [() => null],
    });

    expect(result.html).toBe(markdownToHtml("# Title\n\nText.").html);
  });

  test("a gated-off plugin does not force position tracking on the others", () => {
    const positions: (object | undefined)[] = [];
    const wantsPositions = defineMdastPlugin({
      name: "wants-positions",
      options: { position: true },
      heading() {},
    });
    const observer = defineMdastPlugin({
      name: "observer",
      heading(node) {
        positions.push(node.position);
      },
    });

    markdownToHtml("# Title", { mdastPlugins: [wantsPositions, observer] });
    expect(positions).toHaveLength(1);
    expect(positions[0]).toBeDefined();

    markdownToHtml("# Title", {
      mdastPlugins: [(ctx) => (ctx.sourceFormat === "mdx" ? wantsPositions : null), observer],
    });
    expect(positions).toHaveLength(2);
    expect(positions[1]).toBeUndefined();
  });

  test("a factory is called once per compile even when it skips", () => {
    let calls = 0;
    const factory = () => {
      calls++;
      return null;
    };

    markdownToHtml("# A", { mdastPlugins: [factory] });
    markdownToHtml("# B", { mdastPlugins: [factory] });

    expect(calls).toBe(2);
  });

  test("data written by a factory survives when every plugin skips", () => {
    const data: Record<string, unknown> = {};

    const result = markdownToHtml("# Title", {
      data,
      mdastPlugins: [
        (ctx) => {
          ctx.data.fromFactory = true;
          return null;
        },
      ],
      hastPlugins: [() => false],
    });

    expect(result.data).toBe(data);
    expect(result.data.fromFactory).toBe(true);
  });

  test("a leading BOM is outside the source a factory and a visitor see", () => {
    const seen: string[] = [];

    markdownToHtml("\uFEFF# Title", {
      mdastPlugins: [
        (ctx) => {
          seen.push(ctx.source);
          return defineMdastPlugin({
            name: "reader",
            heading(_node, visitorCtx) {
              seen.push(visitorCtx.source);
            },
          });
        },
      ],
    });

    expect(seen).toEqual(["# Title", "# Title"]);
  });

  test("a factory cannot mutate the context a later factory sees", () => {
    const seen: string[] = [];
    const vandal = (ctx: PluginFactoryContext) => {
      expect(() => {
        (ctx as { source: string }).source = "tampered";
      }).toThrow(TypeError);
      return null;
    };

    markdownToHtml("# Title", {
      mdastPlugins: [
        vandal,
        (ctx) => {
          seen.push(ctx.source);
          return null;
        },
      ],
    });

    expect(seen).toEqual(["# Title"]);
  });
});
