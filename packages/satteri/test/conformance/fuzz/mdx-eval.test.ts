import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  mdxChaos,
  mdxDocument,
  collectMdxEvalIssues,
  deduplicateMdxEvalIssues,
  formatMdxEvalIssue,
  loadCorpus,
  replayMdxEvalCorpus,
  appendCorpus,
} from "./shared.js";

describe("fuzz: MDX eval conformance", () => {
  test("collect and report MDX eval issues", async () => {
    const corpusPath = new URL("./corpus/mdx-eval.txt", import.meta.url);
    const corpus = loadCorpus(corpusPath);

    const allIssues = [
      ...(await replayMdxEvalCorpus(corpus)),
      ...(await collectMdxEvalIssues(mdxDocument, "structured")),
      ...(await collectMdxEvalIssues(mdxChaos, "chaos")),
    ];
    const unique = deduplicateMdxEvalIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# MDX fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatMdxEvalIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES-MDX-EVAL.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      appendCorpus(
        corpusPath,
        unique.filter((i) => i.source !== "corpus").map((i) => i.input),
      );

      const hard = unique.filter((i) => i.kind !== "both-error-disagree");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} MDX conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    }
  });
});
