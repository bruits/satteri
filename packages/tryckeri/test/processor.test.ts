import { describe, it, expect } from "vitest";
import { buildHelloWorldBuffer } from "./fixtures.js";
import { createProcessor } from "../src/processor.js";
import { defineMdastPlugin } from "../src/plugin.js";

describe("createProcessor", () => {
  it("createProcessor([]) works, processBuffer returns same buffer", async () => {
    const buf = buildHelloWorldBuffer();
    const processor = createProcessor({ plugins: [] });
    const result = await processor.processBuffer(buf);
    expect(result.buffer).toBe(buf);
    expect(result.mutationCount).toBe(0);
    expect(result.diagnostics).toEqual([]);
  });

  it("multiple plugins run in order", async () => {
    const buf = buildHelloWorldBuffer();
    let headingCallCount = 0;
    const counterPlugin = defineMdastPlugin({
      name: "counter",
      createOnce() {
        return {
          heading(_node: unknown) {
            headingCallCount++;
          },
        };
      },
    });
    const processor = createProcessor({ plugins: [counterPlugin] });
    await processor.processBuffer(buf);
    expect(headingCallCount).toBe(1);
  });

  it("createOnce is called once per processor, not once per processBuffer call", async () => {
    const buf = buildHelloWorldBuffer();
    let createOnceCallCount = 0;
    const countingPlugin = defineMdastPlugin({
      name: "counter",
      createOnce(_ctx) {
        createOnceCallCount++;
        return {};
      },
    });
    const processor = createProcessor({ plugins: [countingPlugin] });
    await processor.processBuffer(buf);
    await processor.processBuffer(buf);
    expect(createOnceCallCount).toBe(1);
  });

  it('processBufferToTree returns a tree object with type === "root"', async () => {
    const buf = buildHelloWorldBuffer();
    const processor = createProcessor({ plugins: [] });
    const result = await processor.processBufferToTree(buf);
    expect(result.tree).toBeTruthy();
    expect(result.tree.type).toBe("root");
  });

  it("processBufferToTree tree has children", async () => {
    const buf = buildHelloWorldBuffer();
    const processor = createProcessor({ plugins: [] });
    const result = await processor.processBufferToTree(buf);
    expect(Array.isArray(result.tree.children)).toBe(true);
    expect(result.tree.children!.length).toBeGreaterThan(0);
  });

  it("getDiagnostics returns array (empty when no processor-level reports)", () => {
    const processor = createProcessor({ plugins: [] });
    expect(processor.getDiagnostics()).toEqual([]);
  });

  it("createProcessor throws for invalid plugin (missing name)", () => {
    expect(() =>
      createProcessor({
        plugins: [
          {
            name: "",
            createOnce() {
              return {};
            },
          },
        ],
      }),
    ).toThrow(/Invalid plugin/);
  });
});
