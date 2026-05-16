import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  mdxChaos,
  mdxDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  FUZZ_TIMEOUT_MS,
} from "./shared.js";

describe("fuzz: MDX conformance", () => {
  test(
    "collect and report MDX mdast/hast issues",
    () => {
      const allIssues = [
        ...collectIssues(mdxDocument, "mdx-mdast", "structured"),
        ...collectIssues(mdxDocument, "mdx-hast", "structured"),
        ...collectIssues(mdxChaos, "mdx-mdast", "chaos"),
        ...collectIssues(mdxChaos, "mdx-hast", "chaos"),
      ];

      const unique = deduplicateIssues(allIssues);

      const report = [
        "# MDX mdast/hast fuzz-discovered conformance issues",
        "",
        unique.length === 0
          ? "No issues found in the latest run."
          : `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      if (unique.length > 0) {
        const issuesPath = new URL("./FUZZ-ISSUES-MDX.md", import.meta.url);
        writeFileSync(issuesPath, report + "\n");
      }

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} MDX conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    },
    FUZZ_TIMEOUT_MS,
  );
});
