import { describe, test, expect } from "vitest";
import {
  assertDebugBinary,
  assertSliceInvariantEverywhere,
  referenceMdast,
  satteriMdast,
  satteriMdastSkippingFnrAutolink,
} from "./helpers.js";

// GFM autolink literals reach the tree by two routes, and the routes disagree
// on URL decoding, on which domains they accept, and on how far a link may
// run. Matching rendered output is therefore not enough — this probe pins
// which route each input takes.

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
  const withoutFnr = collectLinks(satteriMdastSkippingFnrAutolink(input)).map(linkKey);
  return collectLinks(satteriMdast(input)).map((link) => {
    const ix = withoutFnr.indexOf(linkKey(link));
    if (ix === -1) return "fnr";
    withoutFnr.splice(ix, 1);
    return "construct";
  });
}

// Every shape where the three opener states (still open / closed-and-failed /
// closed-and-resolved) differ, every construct that can swallow a bracket
// before the trigger sees it, and each preceding-character rule.
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

// Each trigger has its own preceding-character rule and they disagree, so the
// interesting cells are the ones where two of them differ. What the construct
// path blocks falls through to find-and-replace, which wants whitespace or
// punctuation.
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

// Deliberate divergence: remark inspects the preceding character as a single
// UTF-16 code unit, so an astral one is judged as a lone surrogate and always
// rejected. Sätteri classifies the whole scalar and accepts astral
// punctuation and symbols. See website/content/docs/divergences.md.
describe("divergence: astral characters before a GFM autolink", () => {
  const ASTRAL_PUNCTUATION_OR_SYMBOL = [
    { prefix: "\u{10101}", name: "U+10101 AEGEAN WORD SEPARATOR DOT (Po)" },
    { prefix: "\u{1F600}", name: "U+1F600 GRINNING FACE (So)" },
    { prefix: "\u{1D6DB}", name: "U+1D6DB MATHEMATICAL BOLD PARTIAL DIFFERENTIAL (Sm)" },
  ];

  test.each(ASTRAL_PUNCTUATION_OR_SYMBOL)("$name starts an autolink", ({ prefix }) => {
    // Unbracketed, only `www.` shows it: the other two take the construct path.
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
    // `Nd`, so both sides reject it — for different reasons, which is the point.
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

  test("skipping the pass changes nothing that has no autolink", () => {
    // The differential signal holds only while the skip is the sole difference.
    for (const input of ["[a](/b) x", "`[` x", "# [a", "text **bold** and `code`"]) {
      expect(satteriMdastSkippingFnrAutolink(input)).toEqual(satteriMdast(input));
    }
  });

  test("every reported span slices back to its source", () => {
    for (const input of PROBE_INPUTS) {
      assertSliceInvariantEverywhere(satteriMdast(input), input);
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

    expect(
      mismatches,
      `${mismatches.length} of ${PROBE_INPUTS.length} inputs take the wrong path`,
    ).toEqual([]);
  });
});
