import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  mathChaos,
  mathDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  FUZZ_TIMEOUT_MS,
} from "./shared.js";

describe("fuzz: math conformance", () => {
  test(
    "collect and report math issues",
    () => {
      const allIssues = [
        ...collectIssues(mathDocument, "math-mdast", "structured"),
        ...collectIssues(mathDocument, "math-hast", "structured"),
        ...collectIssues(mathDocument, "math-html", "structured"),
        ...collectIssues(mathChaos, "math-mdast", "chaos"),
        ...collectIssues(mathChaos, "math-hast", "chaos"),
        ...collectIssues(mathChaos, "math-html", "chaos"),
      ];

      const unique = deduplicateIssues(allIssues);

      const report = [
        "# Math fuzz-discovered conformance issues",
        "",
        unique.length === 0
          ? "No issues found in the latest run."
          : `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      if (unique.length > 0) {
        const issuesPath = new URL("./FUZZ-ISSUES-MATH.md", import.meta.url);
        writeFileSync(issuesPath, report + "\n");
      }

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} math conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    },
    FUZZ_TIMEOUT_MS,
  );
});
