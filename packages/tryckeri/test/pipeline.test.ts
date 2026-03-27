import { test, expect } from "vitest";
import { runPluginsOnBuffer } from "../src/pipeline.js";
import { DataMap } from "../src/data-map.js";
import { buildHelloWorldBuffer } from "./fixtures.js";
import type { MdastNode } from "../src/types.js";

function makePlugin(instance: Record<string, unknown>, name = "test-plugin") {
  return { instance, name };
}

test("structuralMutationCount is 0 for a data-only plugin (heading-ids)", async () => {
  const buffer = buildHelloWorldBuffer();

  const headingIdsPlugin = {
    heading(node: MdastNode) {
      node.data = { id: node.children?.[0]?.value ?? "heading" };
    },
  };

  const result = await runPluginsOnBuffer(buffer, [makePlugin(headingIdsPlugin)]);

  expect(result.structuralMutationCount).toBe(0);
  expect(result.mutationCount).toBeGreaterThanOrEqual(0);
});

test("structuralMutationCount is 1 for a plugin that returns a replacement node", async () => {
  const buffer = buildHelloWorldBuffer();

  const replacePlugin = {
    heading(node: MdastNode) {
      return { type: "paragraph", children: node.children } as unknown as MdastNode;
    },
  };

  const result = await runPluginsOnBuffer(buffer, [makePlugin(replacePlugin)]);

  expect(result.structuralMutationCount).toBe(1);
});

test("mutationCount tracks plugins that produce mutations", async () => {
  const buffer = buildHelloWorldBuffer();

  // plugin1 replaces heading with paragraph — mutation applied
  const plugin1 = {
    heading(node: MdastNode) {
      return { type: "paragraph", children: node.children } as unknown as MdastNode;
    },
  };

  // plugin2 tries to remove headings, but plugin1 already replaced it with a
  // paragraph, so plugin2's heading visitor doesn't fire.
  const plugin2 = {
    heading(node: MdastNode, ctx: { removeNode(n: MdastNode): void }) {
      ctx.removeNode(node);
    },
  };

  const result = await runPluginsOnBuffer(buffer, [makePlugin(plugin1, "p1"), makePlugin(plugin2, "p2")]);

  // Only plugin1 produces a mutation since the heading is gone by the time plugin2 runs
  expect(result.mutationCount).toBe(1);
  expect(result.structuralMutationCount).toBe(1);
});

test("same buffer reference returned when no structural mutations", async () => {
  const buffer = buildHelloWorldBuffer();

  const noopPlugin = {};

  const result = await runPluginsOnBuffer(buffer, [makePlugin(noopPlugin)]);

  expect(result.buffer).toBe(buffer);
  expect(result.structuralMutationCount).toBe(0);
});

test("DataMap entries are visible across plugin passes (plugin 1 sets, plugin 2 reads)", async () => {
  const buffer = buildHelloWorldBuffer();
  let seenIdInPlugin2: string | null = null;

  const plugin1 = {
    heading(node: MdastNode) {
      node.data = { id: "my-heading" };
    },
  };

  const plugin2 = {
    heading(node: MdastNode) {
      seenIdInPlugin2 = (node.data as { id?: string } | null)?.id ?? null;
    },
  };

  const result = await runPluginsOnBuffer(buffer, [makePlugin(plugin1, "p1"), makePlugin(plugin2, "p2")]);

  expect(result.dataMap).toBeInstanceOf(DataMap);
  const nodeData = result.dataMap.get(1);
  expect(nodeData).not.toBeNull();
  expect(nodeData).toHaveProperty("id");
  expect(seenIdInPlugin2).toBe("my-heading");
});

test("filename option is available in plugin fileContext", async () => {
  const buffer = buildHelloWorldBuffer();
  let capturedFilename: string | null = null;

  const plugin = {
    before(fileContext: { filename: string }) {
      capturedFilename = fileContext.filename;
    },
  };

  await runPluginsOnBuffer(buffer, [makePlugin(plugin)], { filename: "my-doc.md" });

  expect(capturedFilename).toBe("my-doc.md");
});

test("empty plugin list returns original buffer and zero mutations", async () => {
  const buffer = buildHelloWorldBuffer();
  const result = await runPluginsOnBuffer(buffer, []);

  expect(result.buffer).toBe(buffer);
  expect(result.mutationCount).toBe(0);
  expect(result.structuralMutationCount).toBe(0);
  expect(result.diagnostics.length).toBe(0);
});

test("provided dataMap is used and returned in result", async () => {
  const buffer = buildHelloWorldBuffer();
  const customDataMap = new DataMap();
  customDataMap.set(99, { "pre-existing": "yes" });

  const result = await runPluginsOnBuffer(buffer, [], { dataMap: customDataMap });

  expect(result.dataMap).toBe(customDataMap);
  const nodeData = result.dataMap.get(99);
  expect(nodeData?.["pre-existing"]).toBe("yes");
});

test("diagnostics are collected from all plugins", async () => {
  const buffer = buildHelloWorldBuffer();

  const plugin1 = {
    before(_fileCtx: unknown, ctx: { report(d: { message: string; severity: string }): void }) {
      ctx.report({ message: "warning from plugin 1", severity: "warning" });
    },
  };
  const plugin2 = {
    before(_fileCtx: unknown, ctx: { report(d: { message: string; severity: string }): void }) {
      ctx.report({ message: "error from plugin 2", severity: "error" });
    },
  };

  const result = await runPluginsOnBuffer(buffer, [makePlugin(plugin1, "p1"), makePlugin(plugin2, "p2")]);

  expect(result.diagnostics.length).toBe(2);
  expect(result.diagnostics.some((d) => d.message === "warning from plugin 1")).toBe(true);
  expect(result.diagnostics.some((d) => d.message === "error from plugin 2")).toBe(true);
});
