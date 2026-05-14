import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  fmChaos,
  fmDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  loadCorpus,
  replayCorpus,
  appendCorpus,
} from "./shared.js";

describe("fuzz: frontmatter conformance", () => {
  test("collect and report frontmatter issues", () => {
    const corpusPath = new URL("./corpus/fm.txt", import.meta.url);
    const corpus = loadCorpus(corpusPath);

    const allIssues = [
      ...replayCorpus(corpus, ["fm-mdast", "fm-hast", "fm-html"]),
      ...collectIssues(fmDocument, "fm-mdast", "structured"),
      ...collectIssues(fmDocument, "fm-hast", "structured"),
      ...collectIssues(fmDocument, "fm-html", "structured"),
      ...collectIssues(fmChaos, "fm-mdast", "chaos"),
      ...collectIssues(fmChaos, "fm-hast", "chaos"),
      ...collectIssues(fmChaos, "fm-html", "chaos"),
    ];

    const unique = deduplicateIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# Frontmatter fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES-FM.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      appendCorpus(
        corpusPath,
        unique.filter((i) => i.source !== "corpus").map((i) => i.input),
      );

      const inputs = unique.map((i) => JSON.stringify(i.input));
      expect
        .soft(
          unique,
          `Found ${unique.length} frontmatter conformance issue(s):\n${inputs.join("\n")}`,
        )
        .toHaveLength(0);
    }
  });
});
