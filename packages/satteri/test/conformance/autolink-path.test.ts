import { describe, test, expect } from "vitest";
import {
  assertMdastConformance,
  assertSliceInvariantEverywhere,
  collectUrls,
  conforms,
  linkUrls,
  referenceMdast,
  satteriMdast,
} from "./helpers.js";
import type { UrlNode } from "./helpers.js";

// Telling satteri's two autolink routes apart needs parser internals, so that
// half of the probe is `autolink_path_probe` in `crates/satteri-pulldown-cmark`.
// These tables are the shared contract: here they pin remark's classification,
// there satteri's.

type PathKind = "construct" | "fnr" | "none";

interface AnyNode {
  type: string;
  children?: AnyNode[];
  position?: unknown;
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

/** remark: a positioned `link` came from micromark, a bare one from findAndReplace. */
function referencePaths(input: string): PathKind[] {
  return collectLinks(referenceMdast(input)).map((link) => (link.position ? "construct" : "fnr"));
}

function referencePath(input: string): PathKind {
  return referencePaths(input)[0] ?? "none";
}

// Every shape where the three opener states (still open, closed-and-failed,
// closed-and-resolved) differ, plus every construct that can swallow a bracket
// before the trigger sees it.
const PROBE_INPUTS: Array<[string, PathKind[]]> = [
  // Bracket-opener states.
  ["[a](/b) www.x.y", ["construct", "construct"]],
  ["[a [b](/c) www.x.y", ["construct", "fnr"]],
  ["[a] www.x.y", ["construct"]],
  ["![a] www.x.y", ["construct"]],
  ["[a www.x.y", ["fnr"]],
  ["[a\nwww.x.y", ["fnr"]],
  ["[a\n\nwww.x.y", ["construct"]],
  ["# [a www.x.y", ["fnr"]],
  // Brackets consumed by an enclosing construct before the trigger.
  ["[a `]` www.x.y", ["fnr"]],
  ["`[` www.x.y", ["construct"]],
  ["[a ``]`` www.x.y", ["fnr"]],
  ["``[`` www.x.y", ["construct"]],
  ["<span a='['> www.x.y", ["construct"]],
  ["[a <http://q.r/]> www.x.y", ["construct", "fnr"]],
  // A trigger inside a link destination the parser has already resolved.
  ["[a](https://x.y)x", ["construct"]],
  ["[a](www.x.y)x", ["construct"]],
  // …and inside one it never resolves, so the trigger sees ordinary bytes.
  ["[[x]](https://x.y)x\n\n[x]: /", ["construct"]],
  ["[[x]](www.a.com)y\n\n[x]: /", ["construct"]],
  ["[foo][bar](https://x.y)x\n\n[bar]: /", ["construct"]],
  ["[[a](/b)](https://x.y)x", ["construct", "construct"]],
  // Unclosed or non-resolving brackets around a trigger.
  ["[www.a.com", ["fnr"]],
  ["[www.a.com]", ["fnr"]],
  ["[www.a.com](", ["fnr"]],
  ["![www.a.com", ["fnr"]],
  ["[foo][www.a.com]", ["fnr"]],
  ["[https://a.com](", ["fnr"]],
  // A `]` balances its opener even when nothing resolves, so a trigger past
  // it is no longer blocked and the URL before it can't run on.
  ["[www.a.com]www.b.com", ["fnr", "construct"]],
  ["[www.a.com]]www.b.com", ["fnr", "construct"]],
  ["[www.a.com]http://b.com", ["fnr", "construct"]],
  ["[www.a.com]u@b.com", ["fnr", "construct"]],
  ["[www.a.com]_u@b.com", ["fnr", "construct"]],
  ["[http://a.com]www.b.com", ["fnr", "construct"]],
  ["a[www.a.com]www.b.com", ["fnr", "construct"]],
  // The opener is still unbalanced, so both triggers stay blocked.
  ["[[www.a.com]www.b.com", ["fnr"]],
  // No opener at all: `]` is an ordinary URL byte.
  ["www.a.com]www.b.com", ["construct"]],
  // Preceding-character rules. `www.` takes a fixed whitelist, `http://`
  // rejects only ASCII letters, and email rejects `/` and atext.
  ["www.x.y", ["construct"]],
  [".www.x.y", ["fnr"]],
  [".http://x.y", ["construct"]],
  ["awww.x.y", []],
  ["5http://x.y", ["construct"]],
  ["/a@b.cd", []],
  ["(www.x.y)", ["construct"]],
  ["_www.x.y_", ["construct"]],
  ["x\u{85}www.x.y", []],
];

// The triggers have disagreeing preceding-character rules, and what the
// construct path blocks falls through to find-and-replace, which wants
// whitespace or punctuation. The fourth is a `www.` literal and an email at
// the same offset, so it also pins which construct is tried first.
const TRIGGERS = ["www.x.y", "http://x.y", "a@b.cd", "www.x@y.zz"] as const;
const PRECEDING_RULES: Array<{
  prefix: string;
  name: string;
  paths: [PathKind, PathKind, PathKind, PathKind];
}> = [
  {
    prefix: "",
    name: "start of document",
    paths: ["construct", "construct", "construct", "construct"],
  },
  { prefix: " ", name: "space", paths: ["construct", "construct", "construct", "construct"] },
  { prefix: "(", name: "`(`", paths: ["construct", "construct", "construct", "construct"] },
  { prefix: "*", name: "`*`", paths: ["construct", "construct", "construct", "construct"] },
  { prefix: "_", name: "`_`", paths: ["construct", "construct", "construct", "construct"] },
  { prefix: "]", name: "`]`", paths: ["construct", "construct", "construct", "construct"] },
  { prefix: "~", name: "`~`", paths: ["construct", "construct", "construct", "construct"] },
  { prefix: "[", name: "`[`", paths: ["fnr", "fnr", "fnr", "fnr"] },
  { prefix: ".", name: "`.`", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: "/", name: "`/`", paths: ["fnr", "construct", "none", "fnr"] },
  { prefix: "+", name: "`+`", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: ")", name: "`)`", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: "!", name: "`!`", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: ":", name: "`:`", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: "¥", name: "BMP symbol", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: "→", name: "BMP math symbol", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: "a", name: "ASCII letter", paths: ["none", "none", "construct", "construct"] },
  { prefix: "5", name: "ASCII digit", paths: ["none", "construct", "construct", "construct"] },
  { prefix: "é", name: "BMP letter", paths: ["none", "construct", "construct", "construct"] },
  { prefix: "你", name: "CJK letter", paths: ["none", "construct", "construct", "construct"] },
  {
    prefix: "你好",
    name: "two CJK letters",
    paths: ["none", "construct", "construct", "construct"],
  },
  {
    prefix: "​",
    name: "zero-width space (Cf)",
    paths: ["none", "construct", "construct", "construct"],
  },
  // U+FEFF is not `White_Space`, yet find-and-replace takes it as a boundary.
  // Prefixed with a letter to keep leading-BOM handling out of it.
  { prefix: "a﻿", name: "byte order mark", paths: ["fnr", "construct", "construct", "construct"] },
  // U+0085 is `White_Space`, but find-and-replace does not take it as a boundary.
  { prefix: "\u{85}", name: "next line", paths: ["none", "construct", "construct", "construct"] },
];

describe("GFM autolink preceding-character rules", () => {
  test.each(PRECEDING_RULES)("$name", ({ prefix, paths }) => {
    for (const [ix, trigger] of TRIGGERS.entries()) {
      expect(referencePath(prefix + trigger), JSON.stringify(prefix + trigger)).toBe(paths[ix]);
    }
  });
});

// Deliberate divergence: remark reads the preceding character as one UTF-16
// code unit, so an astral one is a lone surrogate and always rejected; satteri
// classifies the whole scalar. See website/content/docs/divergences.md.
describe("divergence: astral characters before a GFM autolink", () => {
  // The categories the classifier lets through, so satteri links and remark
  // does not.
  const ACCEPTED = [
    { prefix: "\u{10101}", name: "U+10101 AEGEAN WORD SEPARATOR DOT (Po)" },
    { prefix: "\u{1F600}", name: "U+1F600 GRINNING FACE (So)" },
    { prefix: "\u{1D6DB}", name: "U+1D6DB MATHEMATICAL BOLD PARTIAL DIFFERENTIAL (Sm)" },
    { prefix: "\u{1F468}\u{200d}\u{1F4BB}", name: "a ZWJ sequence ending in U+1F4BB (So)" },
  ];

  // `Nd`, so both sides reject it — for different reasons, which is the point.
  const REJECTED = [{ prefix: "\u{1FBF0}", name: "U+1FBF0 SEGMENTED DIGIT ZERO (Nd)" }];

  // The overlapping trigger is left out: remark's find-and-replace finds the
  // email inside it whatever precedes the `www.`, so it is pinned below.
  const BLOCKED = TRIGGERS.filter((trigger) => trigger !== "www.x@y.zz");

  test.each([...ACCEPTED, ...REJECTED])("$name starts nothing in remark", ({ prefix }) => {
    expect(referencePaths(`${prefix}www.x.y`)).toEqual([]);
    for (const trigger of BLOCKED) {
      expect(referencePaths(`[${prefix}${trigger}`)).toEqual([]);
    }
  });

  test.each(ACCEPTED)("$name: the www half of an overlap still wins", ({ prefix }) => {
    const md = `[${prefix}www.x@y.zz`;
    expect(linkUrls(md)).toEqual(["http://www.x@y.zz"]);
    expect(collectUrls(referenceMdast(md))).toEqual(["mailto:x@y.zz"]);
  });

  test.each(REJECTED)("$name: the email inside the overlap wins on both sides", ({ prefix }) => {
    const md = `[${prefix}www.x@y.zz`;
    expect(linkUrls(md)).toEqual(["mailto:x@y.zz"]);
    expect(collectUrls(referenceMdast(md))).toEqual(["mailto:x@y.zz"]);
  });

  // The span has to stop at the trigger, not swallow the character before it.
  test.each(ACCEPTED)("$name: satteri links, starting at the trigger", ({ prefix }) => {
    const md = `${prefix}www.example.com`;
    expect(linkUrls(md)).toEqual(["http://www.example.com"]);

    const link = collectLinks(satteriMdast(md))[0] as UrlNode;
    const { start, end } = link.position!;
    expect(md.slice(start.offset, end.offset)).toBe("www.example.com");
  });

  // Only the `www` trigger diverges: the other two accept the character on
  // both sides.
  test.each([
    ["\u{1F600}user@example.com\n", ["mailto:user@example.com"]],
    ["\u{1F600}http://example.com\n", ["http://example.com"]],
    ["\u{1F600}_user@example.com\n", ["mailto:_user@example.com"]],
  ])("%j", conforms);
});

describe("GFM autolink path selection", () => {
  test("each input takes the expected path in remark", () => {
    const mismatches = PROBE_INPUTS.filter(
      ([input, expected]) => JSON.stringify(referencePaths(input)) !== JSON.stringify(expected),
    ).map(([input, expected]) => `${JSON.stringify(input)} want [${expected}]`);

    expect(mismatches).toEqual([]);
  });

  test("every reported span slices back to its source", () => {
    for (const [input] of PROBE_INPUTS) {
      assertSliceInvariantEverywhere(satteriMdast(input), input);
    }
  });

  test("each input's tree matches remark", () => {
    for (const [input] of PROBE_INPUTS) {
      assertMdastConformance(input);
    }
  });
});
