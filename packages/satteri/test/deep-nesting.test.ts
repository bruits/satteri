import { spawnSync } from "node:child_process";
import { describe, test, expect } from "vitest";

// A file URL rather than a path: Node cannot import a Windows path (`C:\…`).
const entry = new URL("../dist/index.js", import.meta.url).href;

// A stack overflow kills the whole process, so the compile has to run out of band.
function compileNestedBlockquotes(depth: number) {
  const script = `
    import { markdownToHtml } from ${JSON.stringify(entry)};
    const result = markdownToHtml(">".repeat(${depth}) + " hi");
    process.stdout.write(String(result.html.split("<blockquote>").length - 1));
  `;
  return spawnSync(process.execPath, ["--input-type=module", "-e", script], {
    encoding: "utf8",
  });
}

describe("deeply nested documents", () => {
  test("compiles a shallow blockquote nest", () => {
    const result = compileNestedBlockquotes(5);
    expect(result.signal).toBeNull();
    expect(result.status).toBe(0);
    expect(result.stdout).toBe("5");
  });

  test("compiles a 25000 deep blockquote nest without crashing", () => {
    const result = compileNestedBlockquotes(25000);
    expect(result.signal).toBeNull();
    expect(result.status).toBe(0);
    expect(result.stdout).toBe("25000");
  }, 30_000);
});
