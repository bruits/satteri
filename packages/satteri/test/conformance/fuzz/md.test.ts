import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  chaosString,
  markdownDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  loadCorpus,
  replayCorpus,
  appendCorpus,
} from "./shared.js";

describe("fuzz: conformance", () => {
  test("collect and report all issues", () => {
    const corpusPath = new URL("./corpus/md.txt", import.meta.url);
    const corpus = loadCorpus(corpusPath);

    const allIssues = [
      ...replayCorpus(corpus, ["mdast", "hast", "html"]),
      ...collectIssues(markdownDocument, "mdast", "structured"),
      ...collectIssues(markdownDocument, "hast", "structured"),
      ...collectIssues(markdownDocument, "html", "structured"),
      ...collectIssues(chaosString, "mdast", "chaos"),
      ...collectIssues(chaosString, "hast", "chaos"),
      ...collectIssues(chaosString, "html", "chaos"),
    ];

    const unique = deduplicateIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# Fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      appendCorpus(
        corpusPath,
        unique.filter((i) => i.source !== "corpus").map((i) => i.input),
      );

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    }
  });
});
