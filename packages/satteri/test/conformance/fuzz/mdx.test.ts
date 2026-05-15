import { describe, test, expect } from "vitest";
import { writeFileSync } from "node:fs";
import {
  mdxChaos,
  mdxDocument,
  collectIssues,
  deduplicateIssues,
  formatIssue,
  loadCorpus,
  replayCorpus,
  appendCorpus,
} from "./shared.js";

describe("fuzz: MDX conformance", () => {
  test("collect and report MDX mdast/hast issues", () => {
    const corpusPath = new URL("./corpus/mdx.txt", import.meta.url);
    const corpus = loadCorpus(corpusPath);

    const allIssues = [
      ...replayCorpus(corpus, ["mdx-mdast", "mdx-hast"]),
      ...collectIssues(mdxDocument, "mdx-mdast", "structured"),
      ...collectIssues(mdxDocument, "mdx-hast", "structured"),
      ...collectIssues(mdxChaos, "mdx-mdast", "chaos"),
      ...collectIssues(mdxChaos, "mdx-hast", "chaos"),
    ];

    const unique = deduplicateIssues(allIssues);

    if (unique.length > 0) {
      const report = [
        "# MDX mdast/hast fuzz-discovered conformance issues",
        "",
        `Found ${unique.length} unique issue(s) across ${allIssues.length} total failure(s).`,
        "",
        ...unique.map(formatIssue),
      ].join("\n");

      const issuesPath = new URL("./FUZZ-ISSUES-MDX.md", import.meta.url);
      writeFileSync(issuesPath, report + "\n");

      appendCorpus(
        corpusPath,
        unique.filter((i) => i.source !== "corpus").map((i) => i.input),
      );

      const hard = unique.filter((i) => i.kind !== "position-only");
      const inputs = hard.map((i) => JSON.stringify(i.input));
      expect
        .soft(hard, `Found ${hard.length} MDX conformance issue(s):\n${inputs.join("\n")}`)
        .toHaveLength(0);
    }
  });
});
