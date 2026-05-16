import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  chaosString,
  markdownDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  FUZZ_TIMEOUT_MS,
} from "./shared.js";

describe("fuzz: conformance", () => {
  test(
    "collect and report all issues",
    () => {
      const allIssues = [
        ...collectIssues(markdownDocument, "mdast", "structured"),
        ...collectIssues(markdownDocument, "hast", "structured"),
        ...collectIssues(markdownDocument, "html", "structured"),
        ...collectIssues(chaosString, "mdast", "chaos"),
        ...collectIssues(chaosString, "hast", "chaos"),
        ...collectIssues(chaosString, "html", "chaos"),
      ];

      const unique = deduplicateIssues(allIssues);

      const report = [
        "# Fuzz-discovered conformance issues",
        "",
        unique.length === 0
          ? "No issues found in the latest run."
          : `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      if (unique.length > 0) {
        const issuesPath = new URL("./FUZZ-ISSUES.md", import.meta.url);
        writeFileSync(issuesPath, report + "\n");
      }

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    },
    FUZZ_TIMEOUT_MS,
  );
});
