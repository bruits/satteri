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

// These classifications are shared with the Rust autolink_path_probe tests.

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

// In remark, positions distinguish tokenizer links from find-and-replace links.
function referencePaths(input: string): PathKind[] {
  return collectLinks(referenceMdast(input)).map((link) => (link.position ? "construct" : "fnr"));
}

function referencePath(input: string): PathKind {
  return referencePaths(input)[0] ?? "none";
}

const PROBE_INPUTS: Array<[string, PathKind[]]> = [
  ["[a](/b) www.x.y", ["construct", "construct"]],
  ["[a [b](/c) www.x.y", ["construct", "fnr"]],
  ["[a] www.x.y", ["construct"]],
  ["![a] www.x.y", ["construct"]],
  ["[a www.x.y", ["fnr"]],
  ["[a\nwww.x.y", ["fnr"]],
  ["[a\n\nwww.x.y", ["construct"]],
  ["# [a www.x.y", ["fnr"]],
  ["[a `]` www.x.y", ["fnr"]],
  ["`[` www.x.y", ["construct"]],
  ["[a ``]`` www.x.y", ["fnr"]],
  ["``[`` www.x.y", ["construct"]],
  ["<span a='['> www.x.y", ["construct"]],
  ["[a <http://q.r/]> www.x.y", ["construct", "fnr"]],
  ["[a](https://x.y)x", ["construct"]],
  ["[a](www.x.y)x", ["construct"]],
  ["[[x]](https://x.y)x\n\n[x]: /", ["construct"]],
  ["[[x]](www.a.com)y\n\n[x]: /", ["construct"]],
  ["[foo][bar](https://x.y)x\n\n[bar]: /", ["construct"]],
  ["[[a](/b)](https://x.y)x", ["construct", "construct"]],
  ["[www.a.com", ["fnr"]],
  ["[www.a.com]", ["fnr"]],
  ["[www.a.com](", ["fnr"]],
  ["![www.a.com", ["fnr"]],
  ["[foo][www.a.com]", ["fnr"]],
  ["[https://a.com](", ["fnr"]],
  ["[www.a.com]www.b.com", ["fnr", "construct"]],
  ["[www.a.com]]www.b.com", ["fnr", "construct"]],
  ["[www.a.com]http://b.com", ["fnr", "construct"]],
  ["[www.a.com]u@b.com", ["fnr", "construct"]],
  ["[www.a.com]_u@b.com", ["fnr", "construct"]],
  ["[http://a.com]www.b.com", ["fnr", "construct"]],
  ["a[www.a.com]www.b.com", ["fnr", "construct"]],
  ["[[www.a.com]www.b.com", ["fnr"]],
  ["www.a.com]www.b.com", ["construct"]],
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
  // Prefix U+FEFF with a letter so leading-BOM handling cannot affect this boundary test.
  { prefix: "a﻿", name: "byte order mark", paths: ["fnr", "construct", "construct", "construct"] },
  { prefix: "\u{85}", name: "next line", paths: ["none", "construct", "construct", "construct"] },
];

describe("GFM autolink preceding-character rules", () => {
  test.each(PRECEDING_RULES)("$name", ({ prefix, paths }) => {
    for (const [ix, trigger] of TRIGGERS.entries()) {
      expect(referencePath(prefix + trigger), JSON.stringify(prefix + trigger)).toBe(paths[ix]);
    }
  });
});

// remark classifies one UTF-16 unit; Sätteri classifies the whole preceding Unicode scalar.
describe("divergence: astral characters before a GFM autolink", () => {
  const ACCEPTED = [
    { prefix: "\u{10101}", name: "U+10101 AEGEAN WORD SEPARATOR DOT (Po)" },
    { prefix: "\u{1F600}", name: "U+1F600 GRINNING FACE (So)" },
    { prefix: "\u{1D6DB}", name: "U+1D6DB MATHEMATICAL BOLD PARTIAL DIFFERENTIAL (Sm)" },
    { prefix: "\u{1F468}\u{200d}\u{1F4BB}", name: "a ZWJ sequence ending in U+1F4BB (So)" },
  ];

  const REJECTED = [{ prefix: "\u{1FBF0}", name: "U+1FBF0 SEGMENTED DIGIT ZERO (Nd)" }];

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

  test.each(ACCEPTED)("$name: satteri links, starting at the trigger", ({ prefix }) => {
    const md = `${prefix}www.example.com`;
    expect(linkUrls(md)).toEqual(["http://www.example.com"]);

    const link = collectLinks(satteriMdast(md))[0] as UrlNode;
    const { start, end } = link.position!;
    expect(md.slice(start.offset, end.offset)).toBe("www.example.com");
  });

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
