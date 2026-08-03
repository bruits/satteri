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

/** The single path a lone trigger takes, or `none` when it produces no link. */
function satteriPath(input: string): PathKind | "none" {
  return satteriPaths(input)[0] ?? "none";
}

// Three different preceding-character rules decide whether a trigger fires,
// and they disagree with each other. `previousWww` takes a fixed whitelist,
// `previousProtocol` rejects only ASCII letters, and `previousEmail` rejects
// `/` and atext; whatever they block falls through to find-and-replace's
// `previous()`, which wants whitespace or punctuation. Pinned as a table
// because the interesting cells are the ones where two rules disagree.
const TRIGGERS = ["www.x.y", "http://x.y", "a@b.cd"] as const;
const PRECEDING_RULES: Array<{
  prefix: string;
  name: string;
  paths: [PathKind | "none", PathKind | "none", PathKind | "none"];
}> = [
  { prefix: "", name: "start of document", paths: ["construct", "construct", "construct"] },
  { prefix: " ", name: "space", paths: ["construct", "construct", "construct"] },
  { prefix: "(", name: "`(`", paths: ["construct", "construct", "construct"] },
  { prefix: "*", name: "`*`", paths: ["construct", "construct", "construct"] },
  { prefix: "_", name: "`_`", paths: ["construct", "construct", "construct"] },
  { prefix: "]", name: "`]`", paths: ["construct", "construct", "construct"] },
  { prefix: "~", name: "`~`", paths: ["construct", "construct", "construct"] },
  { prefix: "[", name: "`[`", paths: ["fnr", "fnr", "fnr"] },
  { prefix: ".", name: "`.`", paths: ["fnr", "construct", "construct"] },
  { prefix: "/", name: "`/`", paths: ["fnr", "construct", "none"] },
  { prefix: "+", name: "`+`", paths: ["fnr", "construct", "construct"] },
  { prefix: ")", name: "`)`", paths: ["fnr", "construct", "construct"] },
  { prefix: "!", name: "`!`", paths: ["fnr", "construct", "construct"] },
  { prefix: ":", name: "`:`", paths: ["fnr", "construct", "construct"] },
  { prefix: "¥", name: "BMP symbol", paths: ["fnr", "construct", "construct"] },
  { prefix: "→", name: "BMP math symbol", paths: ["fnr", "construct", "construct"] },
  { prefix: "a", name: "ASCII letter", paths: ["none", "none", "construct"] },
  { prefix: "5", name: "ASCII digit", paths: ["none", "construct", "construct"] },
  { prefix: "é", name: "BMP letter", paths: ["none", "construct", "construct"] },
  { prefix: "你", name: "CJK letter", paths: ["none", "construct", "construct"] },
  { prefix: "你好", name: "two CJK letters", paths: ["none", "construct", "construct"] },
  { prefix: "​", name: "zero-width space (Cf)", paths: ["none", "construct", "construct"] },
  // U+FEFF is not `White_Space`, but JavaScript's `\s` matches it, so
  // find-and-replace treats it as a boundary. Prefixed with a letter to keep
  // the document's own leading-BOM handling out of it.
  { prefix: "a﻿", name: "byte order mark", paths: ["fnr", "construct", "construct"] },
];

describe("GFM autolink preceding-character rules", () => {
  test.each(PRECEDING_RULES)("$name", ({ prefix, paths }) => {
    for (const [ix, trigger] of TRIGGERS.entries()) {
      expect(satteriPath(prefix + trigger), `${JSON.stringify(prefix + trigger)}`).toBe(paths[ix]);
      expect(referencePaths(prefix + trigger)[0] ?? "none").toBe(paths[ix]);
    }
  });
});

// Deliberate divergence, measured against remark-parse@11 + remark-gfm@4
// (mdast-util-gfm-autolink-literal@2.0.1). Its `previous()` reads
// `charCodeAt(index - 1)` — one UTF-16 code unit — so an astral character is
// inspected as a lone surrogate and rejected whatever it is. Sätteri
// classifies the whole scalar, so astral punctuation and symbols are accepted.
// See website/content/docs/divergences.md. If remark ever fixes `previous()`
// these two sides converge and this test is the one to retire.
describe("divergence: astral characters before a GFM autolink", () => {
  const ASTRAL_PUNCTUATION_OR_SYMBOL = [
    { prefix: "\u{10101}", name: "U+10101 AEGEAN WORD SEPARATOR DOT (Po)" },
    { prefix: "\u{1F600}", name: "U+1F600 GRINNING FACE (So)" },
    { prefix: "\u{1D6DB}", name: "U+1D6DB MATHEMATICAL BOLD PARTIAL DIFFERENTIAL (Sm)" },
  ];

  test.each(ASTRAL_PUNCTUATION_OR_SYMBOL)("$name starts an autolink", ({ prefix }) => {
    // Unbracketed, only `www.` shows it: the other two triggers take the
    // construct path, whose own rules never consult `previous()`.
    expect(satteriPath(`${prefix}www.x.y`)).toBe("fnr");
    expect(referencePaths(`${prefix}www.x.y`)).toEqual([]);
    // Behind an unclosed `[` the construct is blocked, so all three triggers
    // fall through to the rule that differs.
    for (const trigger of TRIGGERS) {
      expect(satteriPath(`[${prefix}${trigger}`)).toBe("fnr");
      expect(referencePaths(`[${prefix}${trigger}`)).toEqual([]);
    }
  });

  test("control: an astral character that is neither punctuation nor a symbol", () => {
    // U+1FBF0 SEGMENTED DIGIT ZERO is `Nd`. Both sides reject it, for
    // different reasons, so it must not move when remark's bug is fixed.
    const prefix = "\u{1FBF0}";
    expect(satteriPath(`${prefix}www.x.y`)).toBe("none");
    expect(referencePaths(`${prefix}www.x.y`)).toEqual([]);
    for (const trigger of TRIGGERS) {
      expect(satteriPath(`[${prefix}${trigger}`)).toBe("none");
      expect(referencePaths(`[${prefix}${trigger}`)).toEqual([]);
    }
  });
});

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
