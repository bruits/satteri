import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  mdxMathDocument,
  collectMdxEvalIssues,
  deduplicateMdxEvalIssues,
  formatMdxEvalIssue,
  MDX_MATH_EVAL_OPTIONS,
  FUZZ_TIMEOUT_MS,
} from "./shared.js";

// MDX and math enabled together: differential-fuzz the interaction between
// inline `$...$` spans and `{...}` expressions (the inline-`{` math guard)
// against @mdx-js/mdx + remark-math, rendered to HTML.
describe("fuzz: MDX + math eval conformance", () => {
  test(
    "collect and report MDX+math eval issues",
    async () => {
      const allIssues = [
        ...(await collectMdxEvalIssues(mdxMathDocument, "structured", MDX_MATH_EVAL_OPTIONS)),
      ];
      const unique = deduplicateMdxEvalIssues(allIssues);

      const report = [
        "# MDX + math eval fuzz-discovered conformance issues",
        "",
        unique.length === 0
          ? "No issues found in the latest run."
          : `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatMdxEvalIssue),
      ].join("\n");

      if (unique.length > 0) {
        const issuesPath = new URL("./FUZZ-ISSUES-MDX-MATH.md", import.meta.url);
        writeFileSync(issuesPath, report + "\n");
      }

      const inputs = unique.map((i) => `${i.kind}: ${JSON.stringify(i.input)}`);
      expect
        .soft(unique, `Found ${unique.length} MDX+math conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    },
    FUZZ_TIMEOUT_MS,
  );
});
