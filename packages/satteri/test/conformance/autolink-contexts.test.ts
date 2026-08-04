// Distilled from a hand-crafted GFM autolink differential matrix (3,234 cases,
// families A–L) run against `remark-parse` + `remark-gfm`. Every case kept here is
// one the committed suite did not already exercise, and one whose behaviour no
// other kept case repeats: where the matrix enumerated a rule over a vocabulary,
// only the members that sit on a boundary of that rule survive.
//
// Each row carries the URLs satteri must produce, so a row that stops linking —
// or starts linking something new — fails on its own terms and not only through
// the tree comparison.
//
// This half covers autolinks in context: the constructs that contain them, the
// constructs they collide with, line endings, and bracket state.

import { describe, test, expect } from "vitest";
import {
  assertExtMdastConformance,
  assertMdastConformance,
  collectUrls,
  conforms,
  linkUrls,
  referenceMdast,
  satteriMathMdast,
  satteriMdast,
} from "./helpers.js";
import type { UrlNode } from "./helpers.js";

// Family D — containing constructs. One case per construct, with a second inner
// only where the inner changes the answer.
//
// The two `_`-delimited rows wrap an email whose local part starts with `_`, so the
// opening delimiter and the local part compete for the same character.
describe("family D: containing constructs — emphasis", () => {
  test.each([
    ["*www.example.com*\n", ["http://www.example.com"]],
    ["**www.example.com**\n", ["http://www.example.com"]],
    ["~~www.example.com~~\n", ["http://www.example.com"]],
    ["~www.example.com~\n", ["http://www.example.com"]],
    ["__u@example.com_\n", ["mailto:u@example.com"]],
    ["___u@example.com__\n", ["mailto:u@example.com"]],
    ["*www.example.com\n", ["http://www.example.com"]],
    ["**www.example.com*\n", ["http://www.example.com"]],
    ["a *www.example.com* b\n", ["http://www.example.com"]],
    ["~~a www.example.com~~ b\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family D: containing constructs — code spans", () => {
  test.each([
    ["`www.example.com`\n", []],
    ["``www.example.com``\n", []],
    ["`a` www.example.com\n", ["http://www.example.com"]],
    ["`a www.example.com\n", ["http://www.example.com"]],
    ["a` www.example.com `b\n", []],
  ])("%j", conforms);
});

describe("family D: containing constructs — links and images", () => {
  test.each([
    ["[www.example.com](/dest)\n", ["/dest"]],
    ["[a](www.example.com)\n", ["www.example.com"]],
    ["[a](http://example.com/p)\n", ["http://example.com/p"]],
    ['[a](/dest "www.example.com")\n', ["/dest"]],
    ["[a](<www.example.com>)\n", ["www.example.com"]],
    ["![www.example.com](/dest)\n", []],
    ["![a](www.example.com)\n", []],
    ["[www.example.com][ref]\n\n[ref]: /d\n", []],
    ["[ref][www.example.com]\n\n[ref]: /d\n", ["http://www.example.com"]],
    ["[www.example.com]\n\n[www.example.com]: /d\n", []],
    ["[a]: www.example.com\n\n[a]\n", []],
    ['[a]: /d "www.example.com"\n\n[a]\n', []],
    ["[a][]\n\n[a]: www.example.com\n", []],
  ])("%j", conforms);
});

describe("family D: containing constructs — the `](URL)x` overrun", () => {
  test.each([
    ["[a](www.example.com)x\n", ["www.example.com"]],
    ["[a](/d)www.example.com\n", ["/d", "http://www.example.com"]],
    ["[a](/d)xwww.example.com\n", ["/d"]],
    ["[a](/d)xuser@example.com\n", ["/d", "mailto:xuser@example.com"]],
    ["[a](www.example.com)www.example.com\n", ["www.example.com", "http://www.example.com"]],
    ["[a]( www.example.com )x\n", ["www.example.com"]],
    ['[a](www.example.com"t")x\n', ['www.example.com"t"']],
  ])("%j", conforms);
});

describe("family D: containing constructs — headings", () => {
  test.each([
    ["# www.example.com\n", ["http://www.example.com"]],
    ["### www.example.com ###\n", ["http://www.example.com"]],
    ["www.example.com\n===\n", ["http://www.example.com"]],
    ["www.example.com\n---\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family D: containing constructs — blockquotes", () => {
  test.each([
    ["> www.example.com\n", ["http://www.example.com"]],
    [">www.example.com\n", ["http://www.example.com"]],
    ["> a\nwww.example.com\n", ["http://www.example.com"]],
    ["> > www.example.com\n", ["http://www.example.com"]],
    ["> a\n> user@example.com\n", ["mailto:user@example.com"]],
  ])("%j", conforms);
});

describe("family D: containing constructs — lists", () => {
  test.each([
    ["- www.example.com\n", ["http://www.example.com"]],
    ["1. www.example.com\n", ["http://www.example.com"]],
    ["-   www.example.com\n", ["http://www.example.com"]],
    ["- a\n\n  www.example.com\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family D: containing constructs — tables", () => {
  test.each([
    ["| a |\n| - |\n| www.example.com |\n", ["http://www.example.com"]],
    ["| www.example.com | b |\n| - | - |\n| c | d |\n", ["http://www.example.com"]],
    ["| a |\n| - |\n| www.example.com\\|z |\n", ["http://www.example.com\\|z"]],
    ["| a |\n| - |\n| \\|www.example.com |\n", ["http://www.example.com"]],
    [
      "| a | b |\n| - | - |\n| www.example.com | www.example.com |\n",
      ["http://www.example.com", "http://www.example.com"],
    ],
  ])("%j", conforms);
});

describe("family D: containing constructs — footnotes", () => {
  test.each([
    ["a[^1]\n\n[^1]: www.example.com\n", ["http://www.example.com"]],
    ["a[^www.example.com]\n\n[^www.example.com]: x\n", []],
    ["www.example.com[^1]\n\n[^1]: x\n", ["http://www.example.com[^1"]],
    ["user@example.com[^1]\n\n[^1]: x\n", ["mailto:user@example.com"]],
  ])("%j", conforms);
});

describe("family D: containing constructs — HTML", () => {
  test.each([
    ["<b>www.example.com</b>\n", ["http://www.example.com"]],
    ["a <b> www.example.com\n", ["http://www.example.com"]],
    ["<div>\nwww.example.com\n</div>\n", []],
    ["<!-- www.example.com -->\n", []],
    ["a <!-- www.example.com --> b\n", []],
    ["<?php www.example.com ?>\n", []],
    ['<a href="www.example.com">x</a>\n', []],
  ])("%j", conforms);
});

describe("family D: containing constructs — CommonMark autolinks", () => {
  test.each([
    ["<www.example.com>\n", ["http://www.example.com"]],
    ["<user@example.com>\n", ["mailto:user@example.com"]],
    ["<http://x.y> www.example.com\n", ["http://x.y", "http://www.example.com"]],
    ["www.example.com <http://x.y>\n", ["http://www.example.com", "http://x.y"]],
  ])("%j", conforms);
});

describe("family D: containing constructs — code blocks", () => {
  test.each([
    ["    www.example.com\n", []],
    ["```\nwww.example.com\n```\n", []],
    ["~~~js\nwww.example.com\n~~~\n", []],
    ["```www.example.com\nx\n```\n", []],
  ])("%j", conforms);
});

describe("family D: containing constructs — block boundaries", () => {
  test.each([
    ["www.example.com\n\n---\n", ["http://www.example.com"]],
    ["---\nwww.example.com\n", ["http://www.example.com"]],
    ["www.example.com", ["http://www.example.com"]],
    ["\n\nwww.example.com\n\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family D: containing constructs — math", () => {
  test.each([
    ["$www.example.com$\n", []],
    ["$$www.example.com$$\n", []],
    ["$$\nwww.example.com\n$$\n", []],
    ["a $x$ www.example.com\n", ["http://www.example.com"]],
    ["$a www.example.com\n", ["http://www.example.com"]],
  ])("%j", (md: string, urls: string[]) => {
    assertExtMdastConformance(md, ["math"]);
    expect(collectUrls(satteriMathMdast(md)), JSON.stringify(md)).toEqual(urls);
  });
});

// Family E — shapes where an autolink meets another construct.
describe("family E: adjacent autolinks", () => {
  test.each([
    ["www.a.com www.b.com", ["http://www.a.com", "http://www.b.com"]],
    ["www.a.comwww.b.com", ["http://www.a.comwww.b.com"]],
    ["www.a.com/www.b.com", ["http://www.a.com/www.b.com"]],
    ["http://a.b http://c.d", ["http://a.b", "http://c.d"]],
    ["http://a.bhttp://c.d", ["http://a.bhttp://c.d"]],
    ["http://a.b/http://c.d", ["http://a.b/http://c.d"]],
    ["a@b.com c@d.com", ["mailto:a@b.com", "mailto:c@d.com"]],
    ["a@b.comc@d.com", ["mailto:a@b.comc"]],
    ["www.a.com,www.b.com", ["http://www.a.com,www.b.com"]],
    ["a@b.com,c@d.com", ["mailto:a@b.com", "mailto:c@d.com"]],
  ])("%j", conforms);
});

describe("family E: touching construct delimiters", () => {
  test.each([
    ["*www.a.com*www.b.com*", ["http://www.a.com*www.b.com"]],
    ["www.a.com*www.b.com", ["http://www.a.com*www.b.com"]],
    ["www.a.com_www.b.com", ["http://www.a.com_www.b.com"]],
    ["www.a.com~www.b.com", ["http://www.a.com~www.b.com"]],
    ["[www.a.com](www.b.com)", ["www.b.com"]],
    ["www.a.com[www.b.com]", ["http://www.a.com[www.b.com"]],
    ["`www.a.com`www.b.com", ["http://www.b.com"]],
  ])("%j", conforms);
});

describe("family E: brackets around a trigger", () => {
  test.each([
    ["[www.example.com", ["http://www.example.com"]],
    ["[[www.example.com", ["http://www.example.com"]],
    ["[a][www.example.com", ["http://www.example.com"]],
    ["[a](www.example.com", ["http://www.example.com"]],
    ["]www.example.com", ["http://www.example.com"]],
    ["[www.example.com]", ["http://www.example.com"]],
    ["[www.example.com](", ["http://www.example.com]("]],
    ["![www.example.com", ["http://www.example.com"]],
    ["[a][b] www.example.com", ["http://www.example.com"]],
    ["[](www.example.com)", ["www.example.com"]],
    ["[ www.example.com ]", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family E: escapes", () => {
  test.each([
    ["\\www.example.com", ["http://www.example.com"]],
    ["\\[www.example.com", ["http://www.example.com"]],
    ["www\\.example.com", ["http://www.example.com"]],
    ["www.example\\.com", ["http://www.example\\.com"]],
    ["www.example.com\\", ["http://www.example.com\\"]],
    ["www.example.com\\_x", ["http://www.example.com\\_x"]],
    ["www.example.com\\*x*", ["http://www.example.com\\*x"]],
    ["www.example.com/a\\_b", ["http://www.example.com/a\\_b"]],
    ["a\\@b.com", ["mailto:a@b.com"]],
    ["\\_a@b.com", ["mailto:_a@b.com"]],
    ["_a\\@b.com", ["mailto:_a@b.com"]],
    ["http\\://example.com", ["http://example.com"]],
    ["http:\\//example.com", ["http://example.com"]],
    ["\\\\www.example.com", ["http://www.example.com"]],
    ["\\\\\\www.example.com", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family E: character references", () => {
  test.each([
    ["&#104;ttp://example.com", ["http://example.com"]],
    ["&#119;ww.example.com", ["http://www.example.com"]],
    ["&#95;a@b.com", ["mailto:a@b.com"]],
    ["www.example.com&#47;path", ["http://www.example.com&#47;path"]],
    ["www.example.com&#32;x", ["http://www.example.com&#32;x"]],
    ["www.example&#46;com", ["http://www.example&#46;com"]],
    ["&amp;www.example.com", ["http://www.example.com"]],
    ["&#x77;&#x77;&#x77;.example.com", ["http://www.example.com"]],
    ["&fjlig;www.example.com", []],
    ["&NewLine;www.example.com", ["http://www.example.com"]],
    ["&nbsp;www.example.com", ["http://www.example.com"]],
    ["&copy;www.example.com", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family E: link destinations", () => {
  test.each([
    ["[a](http://x.y)b", ["http://x.y"]],
    ["[a](www.x.y)b", ["www.x.y"]],
    ["[a](u@v.com)b", ["u@v.com"]],
    ["[a](/x)http://y.z", ["/x", "http://y.z"]],
    ["[a](/x)u@v.com", ["/x", "mailto:u@v.com"]],
    ["[a](<http://x.y>)b", ["http://x.y"]],
    ["![a](http://x.y)b", []],
    ["[a](http://x.y)[b](http://c.d)e", ["http://x.y", "http://c.d"]],
    ["[a][b] (http://x.y)c\n\n[b]: /d", ["http://x.y)c"]],
    ["[a[b]c](www.x.y)", ["www.x.y"]],
    ["[a[b](www.x.y)c]", ["www.x.y"]],
    ["[![a](www.x.y)](www.z.w)", ["www.z.w"]],
    ["[a](/b (www.x.y))", ["/b"]],
  ])("%j", conforms);
});

// The rest of the deferred-splice family is in link-edge-cases.test.ts; these
// are the shapes it does not carry.
describe("family E: the deferred splice", () => {
  test.each([
    ["[a] www.x.y\\`code`", ["http://www.x.y\\`code`"]],
    ["<a> www.x.y\\<b>", ["http://www.x.y\\"]],
    ["[a][b] www.x.y\\<c>", ["http://www.x.y\\"]],
    ["$x$ www.a.b", ["http://www.a.b"]],
  ])("%j", conforms);
});

// Family F — line endings.
describe("family F: line endings", () => {
  test.each([
    ["www.example.com\r\n", ["http://www.example.com"]],
    ["x\r\nwww.example.com\r\n", ["http://www.example.com"]],
    ["> x\r\n> www.example.com\r\n", ["http://www.example.com"]],
    ["user@example.com\r\n", ["mailto:user@example.com"]],
    ["[a] www.example.com\r\n", ["http://www.example.com"]],
    ["http://example.com/p.\r\n", ["http://example.com/p"]],
    ["> x\r\n> [a] www.example.com\r\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

// Family I — constructs that shift offsets before the link, including the ones
// that push it onto the find-and-replace path.
describe("family I: position stress", () => {
  test.each([
    ["你好 www.example.com", ["http://www.example.com"]],
    ["你好[www.example.com", ["http://www.example.com"]],
    ["&amp; [www.example.com", ["http://www.example.com"]],
    ["\\* [www.example.com", ["http://www.example.com"]],
    ["> 你好\n> www.example.com", ["http://www.example.com"]],
    ["- [a\n  www.example.com", ["http://www.example.com"]],
    ["[a] &amp; www.x.y", ["http://www.x.y"]],
    ["[a] \t www.x.y", ["http://www.x.y"]],
    ["\t[a] www.x.y", []],
    ["[www.x.y&amp;b", ["http://www.x.y&b"]],
    ["[&fjlig;www.a.com", []],
    ["[www.a.com&fjlig;", ["http://www.a.comfj"]],
    ["[a](/b)\nwww.x.y", ["/b", "http://www.x.y"]],
    ["***[a www.x.y***", ["http://www.x.y"]],
    ["[a `b` www.x.y", ["http://www.x.y"]],
    ["[a <b> www.x.y", ["http://www.x.y"]],
    ["> [a\r\n> www.x.y", ["http://www.x.y"]],
    ["[你好 www.x.y", ["http://www.x.y"]],
    ["[a](/b)你好www.x.y", ["/b"]],
    ["[a www.x.y\n===", ["http://www.x.y"]],
  ])("%j", conforms);
});

// Family K — bracket state and the deferred-autolink decision: a closed `[…]`
// stops blocking a later trigger even when it resolved to nothing, so the URL
// before it must not run on. The separator decides whether the second trigger is
// reachable at all, so the rows below are the label/separator/trigger
// combinations that answer differently, not the full cross-product.
describe("family K: bracket balance — a label that never blocked", () => {
  test.each([
    ["[a]www.b.com\n", ["http://www.b.com"]],
    ["[a]_u@b.com\n", ["mailto:_u@b.com"]],
    ["[a] www.b.com\n", ["http://www.b.com"]],
    ["[a] _u@b.com\n", ["mailto:_u@b.com"]],
    ["[a]xwww.b.com\n", []],
    ["[a]x_u@b.com\n", ["mailto:x_u@b.com"]],
    ["[a](/x)www.b.com\n", ["/x", "http://www.b.com"]],
    ["[a](/x)_u@b.com\n", ["/x", "mailto:_u@b.com"]],
    ["[a](/x) www.b.com\n", ["/x", "http://www.b.com"]],
    ["[a](/x) _u@b.com\n", ["/x", "mailto:_u@b.com"]],
    ["[a](/x)xwww.b.com\n", ["/x"]],
    ["[a](/x)x_u@b.com\n", ["/x", "mailto:x_u@b.com"]],
  ])("%j", conforms);
});

describe("family K: bracket balance — an unbalanced opener still blocks", () => {
  test.each([
    ["[awww.b.com\n", []],
    ["[au@b.com\n", ["mailto:au@b.com"]],
    ["[a_u@b.com\n", ["mailto:a_u@b.com"]],
    ["[a www.b.com\n", ["http://www.b.com"]],
    ["[a u@b.com\n", ["mailto:u@b.com"]],
    ["[a _u@b.com\n", ["mailto:_u@b.com"]],
    ["[axwww.b.com\n", []],
    ["[axu@b.com\n", ["mailto:axu@b.com"]],
    ["[ax_u@b.com\n", ["mailto:ax_u@b.com"]],
    ["[a.www.b.com\n", ["http://www.b.com"]],
    ["[a.u@b.com\n", ["mailto:a.u@b.com"]],
    ["[a._u@b.com\n", ["mailto:a._u@b.com"]],
    ["[a](/xwww.b.com\n", []],
    ["[a](/xu@b.com\n", []],
    ["[a](/x_u@b.com\n", ["mailto:u@b.com"]],
    ["[a](/x www.b.com\n", ["http://www.b.com"]],
    ["[a](/x u@b.com\n", ["mailto:u@b.com"]],
    ["[a](/x _u@b.com\n", ["mailto:_u@b.com"]],
    ["[a](/xxwww.b.com\n", []],
    ["[a](/xxu@b.com\n", []],
    ["[a](/xx_u@b.com\n", ["mailto:u@b.com"]],
    ["[a](/x.www.b.com\n", ["http://www.b.com"]],
    ["[a](/x.u@b.com\n", ["mailto:u@b.com"]],
    ["[a](/x._u@b.com\n", ["mailto:_u@b.com"]],
  ])("%j", conforms);
});

describe("family K: bracket balance — a label ending in a trigger", () => {
  test.each([
    ["[www.a.com] www.b.com\n", ["http://www.a.com", "http://www.b.com"]],
    ["[www.a.com] _u@b.com\n", ["http://www.a.com", "mailto:_u@b.com"]],
    ["[www.a.com]xwww.b.com\n", ["http://www.a.com]xwww.b.com"]],
    ["[www.a.com]xu@b.com\n", ["http://www.a.com", "mailto:xu@b.com"]],
    ["[www.a.com]x_u@b.com\n", ["http://www.a.com", "mailto:x_u@b.com"]],
    ["[www.a.com].u@b.com\n", ["http://www.a.com", "mailto:.u@b.com"]],
    ["[www.a.com]._u@b.com\n", ["http://www.a.com", "mailto:._u@b.com"]],
    ["[www.a.com]-u@b.com\n", ["http://www.a.com", "mailto:-u@b.com"]],
    ["[www.a.com]-_u@b.com\n", ["http://www.a.com", "mailto:-_u@b.com"]],
    ["[http://a.com]u@b.com\n", ["http://a.com", "mailto:u@b.com"]],
    ["[http://a.com]_u@b.com\n", ["http://a.com", "mailto:_u@b.com"]],
    ["[http://a.com] u@b.com\n", ["http://a.com", "mailto:u@b.com"]],
    ["[http://a.com] _u@b.com\n", ["http://a.com", "mailto:_u@b.com"]],
    ["[http://a.com]xwww.b.com\n", ["http://a.com]xwww.b.com"]],
    ["[http://a.com].u@b.com\n", ["http://a.com", "mailto:.u@b.com"]],
    ["[http://a.com]._u@b.com\n", ["http://a.com", "mailto:._u@b.com"]],
    ["[http://a.com]-u@b.com\n", ["http://a.com", "mailto:-u@b.com"]],
    ["[http://a.com]-_u@b.com\n", ["http://a.com", "mailto:-_u@b.com"]],
    ["[u@a.com]_u@b.com\n", ["mailto:u@a.com", "mailto:_u@b.com"]],
    ["[u@a.com]xwww.b.com\n", ["mailto:u@a.com"]],
    ["[u@a.com].u@b.com\n", ["mailto:u@a.com", "mailto:.u@b.com"]],
    ["[u@a.com]._u@b.com\n", ["mailto:u@a.com", "mailto:._u@b.com"]],
    ["[u@a.com]-u@b.com\n", ["mailto:u@a.com", "mailto:-u@b.com"]],
    ["[u@a.com]-_u@b.com\n", ["mailto:u@a.com", "mailto:-_u@b.com"]],
  ])("%j", conforms);
});

describe("family K: a closed label and a second trigger", () => {
  test.each([
    [
      "[www.a.com]www.b.com www.c.com\n",
      ["http://www.a.com", "http://www.b.com", "http://www.c.com"],
    ],
    ["[www.a.com]www.b.com]www.c.com\n", ["http://www.a.com", "http://www.b.com]www.c.com"]],
    [
      "[www.a.com]www.b.com\n\n[www.d.com]www.e.com\n",
      ["http://www.a.com", "http://www.b.com", "http://www.d.com", "http://www.e.com"],
    ],
    ["*[www.a.com]www.b.com*\n", ["http://www.a.com", "http://www.b.com"]],
    ["[www.a.com]www.b.com](/x)\n", ["http://www.a.com", "http://www.b.com"]],
    ["[[www.a.com]www.b.com]www.c.com\n", ["http://www.a.com]www.b.com", "http://www.c.com"]],
    ["[a][www.b.com]www.c.com\n", ["http://www.b.com", "http://www.c.com"]],
    ["![www.a.com]www.b.com\n", ["http://www.a.com", "http://www.b.com"]],
    ["[www.a.com\\]www.b.com\n", ["http://www.a.com]www.b.com"]],
    ["[www.a.com]www.b.com`c`\n", ["http://www.a.com", "http://www.b.com`c`"]],
  ])("%j", conforms);
});

// Family L — reference definitions and footnote definitions.
describe("family L: definitions", () => {
  test.each([
    ["[a]: /x 'www.example.com'\n\n[a]\n", []],
    ["[www.example.com]: /x\n\n[www.example.com]\n", []],
    ["[a]: /x\nwww.example.com\n", ["http://www.example.com"]],
    ["[a]: /x\n\nwww.example.com\n", ["http://www.example.com"]],
    ["[^1]: www.example.com\n\na[^1]\n", ["http://www.example.com"]],
    ["[^www.a.com]: x\n\na[^www.a.com]\n", []],
    ["[a]: <www.example.com>\n\n[a]\n", []],
    ["www.example.com\n[a]: /x\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

// Deliberate divergence (see website/content/docs/divergences.md):
// mdast-util-gfm-task-list-item pulls a task item's paragraph start back over
// the checkbox only when the paragraph's first child is a text node. An
// autolink first child is the case the doc's examples do not name.
describe("family D: paragraph start in a task list item (documented divergence)", () => {
  test("an autolink first child keeps satteri's uniform start", () => {
    const md = "- [ ] www.example.com\n";
    const paragraphStart = (tree: unknown): number => {
      const list = (tree as UrlNode).children![0]!;
      return list.children![0]!.children![0]!.position!.start.offset;
    };
    expect(paragraphStart(satteriMdast(md))).toBe(6);
    expect(paragraphStart(referenceMdast(md))).toBe(2);
    expect(linkUrls(md)).toEqual(["http://www.example.com"]);
  });

  test("a text first child agrees on both sides", () => {
    assertMdastConformance("- [ ] plain text\n");
  });
});
