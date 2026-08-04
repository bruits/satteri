import { test } from "vitest";
import { referenceMdast, satteriMdast } from "../conformance/helpers.js";

const CASES: string[] = (await import(process.env.DIFF_CASES!)).default;

function summarize(node: any, depth = 0): string {
  if (!node || typeof node !== "object") return String(node);
  const p = node.position;
  const pos = p
    ? `${p.start.line}:${p.start.column}(${p.start.offset})..${p.end.line}:${p.end.column}(${p.end.offset})`
    : "no-pos";
  const extra =
    typeof node.value === "string"
      ? ` ${JSON.stringify(node.value)}`
      : node.url
        ? ` url=${JSON.stringify(node.url)}`
        : "";
  let out = `${"  ".repeat(depth)}${node.type} ${pos}${extra}\n`;
  for (const c of node.children ?? []) out += summarize(c, depth + 1);
  return out;
}

test("diff", async () => {
  let out = "";
  for (const md of CASES) {
    const ref = summarize(referenceMdast(md));
    const sat = summarize(satteriMdast(md));
    const same = ref === sat;
    out += `\n=== ${JSON.stringify(md)} ${same ? "SAME" : "DIFF"}\n`;
    if (!same) out += "--- remark\n" + ref + "--- satteri\n" + sat;
  }
  const { writeFileSync } = await import("node:fs");
  writeFileSync(process.env.DIFF_OUT!, out);
});
