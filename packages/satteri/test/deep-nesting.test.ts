import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, test, expect } from "vitest";

const entry = fileURLToPath(new URL("../dist/index.js", import.meta.url));

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

  // Unprotected, an optimized build overflows Node's 8 MB main stack at ~10250.
  test("compiles a 25000 deep blockquote nest without crashing", () => {
    const result = compileNestedBlockquotes(25000);
    expect(result.signal).toBeNull();
    expect(result.status).toBe(0);
    expect(result.stdout).toBe("25000");
  }, 30_000);
});
