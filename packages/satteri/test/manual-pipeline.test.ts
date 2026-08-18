import { describe, test, expect } from "vitest";
import {
  applyCommandsToMdastHandle,
  convertMdastToHastHandle,
  createMdastHandle,
  defineHastPlugin,
  defineMdastPlugin,
  dropHandle,
  getHandleSource,
  markdownToHtml,
  normalizePlugins,
  renderHandle,
  resolveHastSubscriptions,
  resolveMdastSubscriptions,
  visitHastHandle,
  visitHastHook,
  visitMdastHandle,
  visitMdastHook,
} from "../src/index.js";
import type {
  Data,
  HastDiagnostic,
  HastHookFn,
  HastPluginDefinition,
  HastPluginEntry,
  MdastDiagnostic,
  MdastHookFn,
  MdastPluginDefinition,
  MdastPluginEntry,
} from "../src/index.js";

function passesOf<H>(
  plugin: { before?: H; after?: H },
  subscriptionCount: number,
): (H | "visitors")[] {
  const passes: (H | "visitors")[] = [];
  if (typeof plugin.before === "function") passes.push(plugin.before);
  if (subscriptionCount > 0) passes.push("visitors");
  if (typeof plugin.after === "function") passes.push(plugin.after);
  return passes;
}

/** Mirrors the step-by-step pipeline the playground drives by hand. */
function manualPipeline(
  source: string,
  options: {
    mdastPlugins?: MdastPluginEntry[];
    hastPlugins?: HastPluginEntry[];
    data?: Data;
  } = {},
): { html: string; data: Data } {
  const data = options.data ?? {};
  const mdastPlugins = normalizePlugins<MdastPluginDefinition>(
    options.mdastPlugins ?? [],
    "mdastPlugins",
    source,
    undefined,
    "markdown",
    data,
  );
  const hastPlugins = normalizePlugins<HastPluginDefinition>(
    options.hastPlugins ?? [],
    "hastPlugins",
    source,
    undefined,
    "markdown",
    data,
  );

  const mdastHandle = createMdastHandle(source);
  const handleSource = () => getHandleSource(mdastHandle);
  for (const plugin of mdastPlugins) {
    const subs = resolveMdastSubscriptions(plugin);
    const diagnostics: MdastDiagnostic[] = [];
    for (const pass of passesOf<MdastHookFn>(plugin, subs.length)) {
      const result =
        pass === "visitors"
          ? visitMdastHandle(
              mdastHandle,
              plugin,
              subs,
              handleSource,
              undefined,
              data,
              "markdown",
              diagnostics,
            )
          : visitMdastHook(
              mdastHandle,
              plugin,
              pass,
              handleSource,
              undefined,
              data,
              "markdown",
              diagnostics,
            );
      if (result instanceof Promise) throw new Error("async plugin in a sync pipeline");
      if (result.hasMutations) applyCommandsToMdastHandle(mdastHandle, result.commandBuffer);
    }
  }

  const hastHandle = convertMdastToHastHandle(mdastHandle);
  for (const plugin of hastPlugins) {
    const subs = resolveHastSubscriptions(plugin);
    const diagnostics: HastDiagnostic[] = [];
    for (const pass of passesOf<HastHookFn>(plugin, subs.length)) {
      const result =
        pass === "visitors"
          ? visitHastHandle(
              hastHandle,
              plugin,
              subs,
              source,
              undefined,
              data,
              "markdown",
              diagnostics,
            )
          : visitHastHook(
              hastHandle,
              plugin,
              pass,
              source,
              undefined,
              data,
              "markdown",
              diagnostics,
            );
      if (result instanceof Promise) throw new Error("async plugin in a sync pipeline");
    }
  }

  const html = renderHandle(hastHandle);
  dropHandle(hastHandle);
  return { html, data };
}

const SOURCE = "# Hello World\n\nA paragraph.\n";

describe("manual pipeline", () => {
  test("hast before and after hooks reach the tree", () => {
    const plugins = [
      defineHastPlugin({
        name: "hooked",
        before(root, ctx) {
          ctx.prependChild(root, { type: "text", value: "before" });
        },
        element: {
          filter: ["h1"],
          visit(node, ctx) {
            ctx.appendChild(node, { type: "text", value: "!" });
          },
        },
        after(root, ctx) {
          ctx.appendChild(root, { type: "text", value: "after" });
        },
      }),
    ];

    const manual = manualPipeline(SOURCE, { hastPlugins: plugins });
    expect(manual.html).toContain("before");
    expect(manual.html).toContain("Hello World!");
    expect(manual.html).toContain("after");
    expect(manual.html).toBe(markdownToHtml(SOURCE, { hastPlugins: plugins }).html);
  });

  test("mdast before and after hooks reach the tree", () => {
    const plugins = [
      defineMdastPlugin({
        name: "hooked",
        before(root, ctx) {
          ctx.prependChild(root, {
            type: "paragraph",
            children: [{ type: "text", value: "before" }],
          });
        },
        heading(node, ctx) {
          ctx.appendChild(node, { type: "text", value: "!" });
        },
        after(root, ctx) {
          ctx.appendChild(root, {
            type: "paragraph",
            children: [{ type: "text", value: "after" }],
          });
        },
      }),
    ];

    const manual = manualPipeline(SOURCE, { mdastPlugins: plugins });
    expect(manual.html).toBe(markdownToHtml(SOURCE, { mdastPlugins: plugins }).html);
    expect(manual.html).toContain("<p>before</p>");
    expect(manual.html).toContain("Hello World!");
    expect(manual.html).toContain("<p>after</p>");
  });

  test("a hook-only plugin still runs", () => {
    let calls = 0;
    manualPipeline(SOURCE, {
      hastPlugins: [
        defineHastPlugin({
          name: "hook-only",
          before: () => void calls++,
          after: () => void calls++,
        }),
      ],
    });
    expect(calls).toBe(2);
  });

  test("normalizePlugins resolves factories, nested lists, and opt-outs", () => {
    const seen: string[] = [];
    const plugins: HastPluginEntry[] = [
      (ctx) => {
        seen.push(ctx.sourceFormat, ctx.source);
        return [
          defineHastPlugin({
            name: "from-factory",
            after: (root, hookCtx) => hookCtx.appendChild(root, { type: "text", value: "factory" }),
          }),
        ];
      },
      null,
      false,
      () => undefined,
    ];

    const manual = manualPipeline(SOURCE, { hastPlugins: plugins });
    expect(seen).toEqual(["markdown", SOURCE]);
    expect(manual.html).toContain("factory");
  });

  test("data is one bag across the mdast and hast sides", () => {
    const manual = manualPipeline(SOURCE, {
      mdastPlugins: [
        defineMdastPlugin({
          name: "writer",
          before(_root, ctx) {
            ctx.data.headings = 0;
          },
          heading(_node, ctx) {
            ctx.data.headings = (ctx.data.headings as number) + 1;
          },
        }),
      ],
      hastPlugins: [
        defineHastPlugin({
          name: "reader",
          after(root, ctx) {
            ctx.appendChild(root, { type: "text", value: `headings:${ctx.data.headings}` });
          },
        }),
      ],
    });
    expect(manual.data.headings).toBe(1);
    expect(manual.html).toContain("headings:1");
  });

  test("diagnostics reported in before are visible in after", () => {
    let seen: string[] = [];
    manualPipeline(SOURCE, {
      hastPlugins: [
        defineHastPlugin({
          name: "reporter",
          before(_root, ctx) {
            ctx.report({ message: "from before", severity: "warning" });
          },
          after(_root, ctx) {
            seen = ctx.getDiagnostics().map((d) => d.message);
          },
        }),
      ],
    });
    expect(seen).toEqual(["from before"]);
  });
});
