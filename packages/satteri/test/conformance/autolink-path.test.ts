import { describe, test, expect } from "vitest";
import { assertDebugBinary, referenceMdast, satteriMdast, satteriMdastDebug } from "./helpers.js";

// GFM autolink literals reach the tree by two different routes, and which one
// fires is observable: micromark's tokenizer produces a `link` node with a
// position, while `mdast-util-gfm-autolink-literal`'s find-and-replace
// transform produces one without. Matching remark's *output* is not enough —
// the two routes disagree on URLs (raw vs decoded), on which domains they
// accept, and on how far a link may run — so this probe pins the route itself.
//
// Reference side: position presence is the signal, and stays valid because
// remark isn't being modified.
//
// Sätteri side: a differential parse. Each input is parsed twice, once
// normally and once with the find-and-replace post-pass skipped; a `link` that
// survives the second parse came from the first-pass construct scanner. That
// is exact rather than inferential, and it keeps working once Sätteri gives
// find-and-replace nodes positions of their own.

type PathKind = "construct" | "fnr";

interface AnyNode {
  type: string;
  url?: string;
  children?: AnyNode[];
  position?: { start: { offset: number }; end: { offset: number } };
}

function collectLinks(tree: unknown): AnyNode[] {
  const out: AnyNode[] = [];
  const walk = (node: AnyNode): void => {
    if (node.type === "link") out.push(node);
    for (const child of node.children ?? []) walk(child);
  };
  walk(tree as AnyNode);
  return out;
}

function linkKey(node: AnyNode): string {
  const start = node.position ? node.position.start.offset : "-";
  const end = node.position ? node.position.end.offset : "-";
  return `${node.url ?? ""}\0${start}\0${end}`;
}

/** remark: a positioned `link` came from micromark, a bare one from findAndReplace. */
function referencePaths(input: string): PathKind[] {
  return collectLinks(referenceMdast(input)).map((link) => (link.position ? "construct" : "fnr"));
}

/** Sätteri: a `link` that survives with the find-and-replace pass off is a construct link. */
function satteriPaths(input: string): PathKind[] {
  const withoutFnr = collectLinks(satteriMdastDebug(input, { skipFnrAutolink: true })).map(linkKey);
  return collectLinks(satteriMdast(input)).map((link) => {
    const ix = withoutFnr.indexOf(linkKey(link));
    if (ix === -1) return "fnr";
    withoutFnr.splice(ix, 1);
    return "construct";
  });
}

// Drawn from micromark's own semantics: every shape where the three opener
// states (still open / closed-and-failed / closed-and-resolved) differ, every
// construct that can swallow a bracket before the trigger sees it, and the
// three preceding-character rules (`previousWww`, `previousProtocol`,
// `previousEmail`, each with its own accept set).
const PROBE_INPUTS = [
  // Bracket-opener states.
  "[a](/b) www.x.y",
  "[a [b](/c) www.x.y",
  "[a] www.x.y",
  "![a] www.x.y",
  "[a www.x.y",
  "[a\nwww.x.y",
  "[a\n\nwww.x.y",
  "# [a www.x.y",
  // Brackets consumed by an enclosing construct before the trigger.
  "[a `]` www.x.y",
  "`[` www.x.y",
  "[a ``]`` www.x.y",
  "``[`` www.x.y",
  "<span a='['> www.x.y",
  "[a <http://q.r/]> www.x.y",
  // A trigger inside a link destination the parser has already resolved.
  "[a](https://x.y)x",
  "[a](www.x.y)x",
  // …and inside one it never resolves, so the trigger sees ordinary bytes.
  "[[x]](https://x.y)x\n\n[x]: /",
  "[[x]](www.a.com)y\n\n[x]: /",
  "[foo][bar](https://x.y)x\n\n[bar]: /",
  "[[a](/b)](https://x.y)x",
  // Unclosed or non-resolving brackets around a trigger.
  "[www.a.com",
  "[www.a.com]",
  "[www.a.com](",
  "![www.a.com",
  "[foo][www.a.com]",
  "[https://a.com](",
  // Preceding-character rules. `www.` takes a fixed whitelist, `http://`
  // rejects only ASCII letters, and email rejects `/` and atext.
  "www.x.y",
  ".www.x.y",
  ".http://x.y",
  "awww.x.y",
  "5http://x.y",
  "/a@b.cd",
  "(www.x.y)",
  "_www.x.y_",
];

describe("GFM autolink path selection", () => {
  test("the probe measures the parser, not a release binary", () => {
    assertDebugBinary();
  });

  test("skipFnrAutolink is a pure pass-skip", () => {
    // The differential signal is only trustworthy while the debug entry point
    // with no knobs set is the ordinary parse.
    for (const input of PROBE_INPUTS) {
      expect(satteriMdastDebug(input, {})).toEqual(satteriMdast(input));
    }
  });

  test("each autolink takes the same path as in remark", () => {
    assertDebugBinary();
    const mismatches: string[] = [];
    for (const input of PROBE_INPUTS) {
      const expected = referencePaths(input);
      const actual = satteriPaths(input);
      if (JSON.stringify(expected) !== JSON.stringify(actual)) {
        mismatches.push(
          `${JSON.stringify(input)}  ref=[${expected.join(",")}] sat=[${actual.join(",")}]`,
        );
      }
    }

    // Recorded baseline: 10 of 34 inputs mismatch, in two families. Six count
    // brackets over raw bytes, so a `[` or `]` that belongs to a code span,
    // inline HTML or a pointed autolink is miscounted; four are autolinks that
    // overrun a `)` closing a destination the parser never resolved. Kept as a
    // ceiling so the count can only go down.
    expect(mismatches.length).toBeLessThanOrEqual(10);
    expect
      .soft(mismatches, `${mismatches.length} of ${PROBE_INPUTS.length} inputs take the wrong path`)
      .toEqual([]);
  });
});
