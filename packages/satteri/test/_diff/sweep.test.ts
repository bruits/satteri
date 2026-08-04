import { test } from "vitest";
import { referenceMdast, satteriMdast } from "../conformance/helpers.js";
import { writeFileSync } from "node:fs";

function canon(v: any): any {
  if (Array.isArray(v)) return v.map(canon);
  if (v && typeof v === "object") {
    const out: any = {};
    for (const k of Object.keys(v).sort()) out[k] = canon(v[k]);
    return out;
  }
  return v;
}

const BASES = [
  "abc\n",
  "a b\n",
  "# h\n",
  "## h ##\n",
  "h\n===\n",
  "- a\n- b\n",
  "1. a\n2. b\n",
  "- [ ] a\n",
  "> q\n> r\n",
  "```js\nx\n```\n",
  "~~~\ny\n~~~\n",
  "    code\n",
  "| a | b |\n| - | - |\n| 1 | 2 |\n",
  "[a]: /x \"t\"\n\n[a]\n",
  "[l](/x)\n",
  "[l](/x \"t\")\n",
  "![i](/x)\n",
  "*e* **s** `c`\n",
  "~~st~~\n",
  "<div>a</div>\n",
  "<span>a</span> b\n",
  "a\\*b\n",
  "a  \nb\n",
  "---\n",
  "a\n\nb\n",
  "www.a.com b\n",
  "u@e.com x\n",
  "<https://a.com>\n",
  "a[^1]\n\n[^1]: n\n",
  "- a\n\n  b\n",
  "  - a\n    - b\n",
  "> - a\n> - b\n",
  "| a |\n| :-: |\n| b |\n",
  "<!-- c -->\n",
  "<?php x ?>\n",
  "a &amp; b\n",
  "a\\\nb\n",
  "***\n",
  "> a\nb\n",
  "- a\n  - b\n\n- c\n",
  "\ta\n",
  "[a][b]\n\n[b]: /x\n",
  "![a][b]\n\n[b]: /x\n",
  "# h #\n",
  "a\n---\n",
  "1) a\n",
  "<a href=\"/x\">l</a>\n",
  "term\n: def\n",
  "```\n\n```\n",
  "a\n\n\nb\n",
];

test("VT/FF injection sweep", () => {
  const report: string[] = [];
  let n = 0;
  for (const base of BASES) {
    for (const ch of ["", ""]) {
      for (let i = 0; i <= base.length; i++) {
        const md = base.slice(0, i) + ch + base.slice(i);
        n++;
        let ref: string, sat: string;
        try {
          ref = JSON.stringify(canon(referenceMdast(md)));
        } catch (e) {
          continue;
        }
        try {
          sat = JSON.stringify(canon(satteriMdast(md)));
        } catch (e) {
          report.push(`THROW ${JSON.stringify(md)}: ${e}`);
          continue;
        }
        if (ref !== sat) report.push(`DIFF ${JSON.stringify(md)}\n  ref ${ref}\n  sat ${sat}`);
      }
    }
  }
  writeFileSync(process.env.DIFF_OUT!, `${n} cases, ${report.length} diffs\n\n${report.join("\n")}`);
});
