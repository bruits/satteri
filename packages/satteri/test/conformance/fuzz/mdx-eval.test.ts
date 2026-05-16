import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  mdxChaos,
  mdxDocument,
  collectMdxEvalIssues,
  deduplicateMdxEvalIssues,
  formatMdxEvalIssue,
  FUZZ_TIMEOUT_MS,
} from "./shared.js";

describe("fuzz: MDX eval conformance", () => {
  test(
    "collect and report MDX eval issues",
    async () => {
      const allIssues = [
        ...(await collectMdxEvalIssues(mdxDocument, "structured")),
        ...(await collectMdxEvalIssues(mdxChaos, "chaos")),
      ];
      const unique = deduplicateMdxEvalIssues(allIssues);

      const report = [
        "# MDX eval fuzz-discovered conformance issues",
        "",
        unique.length === 0
          ? "No issues found in the latest run."
          : `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatMdxEvalIssue),
      ].join("\n");

      if (unique.length > 0) {
        const issuesPath = new URL("./FUZZ-ISSUES-MDX-EVAL.md", import.meta.url);
        writeFileSync(issuesPath, report + "\n");
      }

      // All kinds count as failures, including `both-error-disagree`
      // (mdx-js rejects, satteri evaluates) — those are real coverage
      // gaps unless explicitly listed in `KNOWN_MDX_EVAL_DIVERGENCES`.
      const inputs = unique.map((i) => `${i.kind}: ${JSON.stringify(i.input)}`);
      expect
        .soft(unique, `Found ${unique.length} MDX conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    },
    FUZZ_TIMEOUT_MS,
  );
});
