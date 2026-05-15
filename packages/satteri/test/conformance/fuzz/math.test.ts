import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  mathChaos,
  mathDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  loadCorpus,
  replayCorpus,
  appendCorpus,
} from "./shared.js";

describe("fuzz: math conformance", () => {
  test("collect and report math issues", () => {
    const corpusPath = new URL("./corpus/math.txt", import.meta.url);
    const corpus = loadCorpus(corpusPath);

    const allIssues = [
      ...replayCorpus(corpus, ["math-mdast", "math-hast", "math-html"]),
      ...collectIssues(mathDocument, "math-mdast", "structured"),
      ...collectIssues(mathDocument, "math-hast", "structured"),
      ...collectIssues(mathDocument, "math-html", "structured"),
      ...collectIssues(mathChaos, "math-mdast", "chaos"),
      ...collectIssues(mathChaos, "math-hast", "chaos"),
      ...collectIssues(mathChaos, "math-html", "chaos"),
    ];

    const unique = deduplicateIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# Math fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES-MATH.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      appendCorpus(
        corpusPath,
        unique.filter((i) => i.source !== "corpus").map((i) => i.input),
      );

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} math conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    }
  });
});
