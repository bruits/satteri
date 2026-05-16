import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  fmChaos,
  fmDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  FUZZ_TIMEOUT_MS,
} from "./shared.js";

describe("fuzz: frontmatter conformance", () => {
  test(
    "collect and report frontmatter issues",
    () => {
      const allIssues = [
        ...collectIssues(fmDocument, "fm-mdast", "structured"),
        ...collectIssues(fmDocument, "fm-hast", "structured"),
        ...collectIssues(fmDocument, "fm-html", "structured"),
        ...collectIssues(fmChaos, "fm-mdast", "chaos"),
        ...collectIssues(fmChaos, "fm-hast", "chaos"),
        ...collectIssues(fmChaos, "fm-html", "chaos"),
      ];

      const unique = deduplicateIssues(allIssues);

      const report = [
        "# Frontmatter fuzz-discovered conformance issues",
        "",
        unique.length === 0
          ? "No issues found in the latest run."
          : `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      if (unique.length > 0) {
        const issuesPath = new URL("./FUZZ-ISSUES-FM.md", import.meta.url);
        writeFileSync(issuesPath, report + "\n");
      }

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} frontmatter conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    },
    FUZZ_TIMEOUT_MS,
  );
});
