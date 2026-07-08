import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  autolinkChaos,
  autolinkDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  FUZZ_TIMEOUT_MS,
} from "./shared.js";

describe("fuzz: GFM autolink conformance", () => {
  test(
    "collect and report autolink issues",
    () => {
      const allIssues = [
        ...collectIssues(autolinkDocument, "mdast", "structured"),
        ...collectIssues(autolinkDocument, "hast", "structured"),
        ...collectIssues(autolinkDocument, "html", "structured"),
        ...collectIssues(autolinkChaos, "mdast", "chaos"),
        ...collectIssues(autolinkChaos, "hast", "chaos"),
        ...collectIssues(autolinkChaos, "html", "chaos"),
      ];

      const unique = deduplicateIssues(allIssues);

      const report = [
        "# Autolink fuzz-discovered conformance issues",
        "",
        unique.length === 0
          ? "No issues found in the latest run."
          : `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      if (unique.length > 0) {
        const issuesPath = new URL("./FUZZ-ISSUES-AUTOLINK.md", import.meta.url);
        writeFileSync(issuesPath, report + "\n");
      }

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} autolink conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    },
    FUZZ_TIMEOUT_MS,
  );
});
