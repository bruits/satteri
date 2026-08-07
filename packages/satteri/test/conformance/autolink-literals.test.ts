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
// This half covers the scanner itself: what may precede a trigger, where the
// URL ends, and the spec's own examples.

import { describe, test, expect } from "vitest";
import { assertMdastConformance, conforms, linkUrls } from "./helpers.js";

// Family A — the preceding-character classifier. It asks a character for its
// Unicode General_Category, so there is one row per category it must accept or
// reject, plus the ASCII characters that mean something else to a neighbouring
// construct; not one row per character in the matrix. Which of the two autolink
// paths each case takes is pinned in autolink-path.test.ts — what is pinned here
// is the tree that comes out, for trigger kinds that file does not carry.
const A_TRIGGERS = [
  "www.example.com",
  "http://example.com",
  "user@example.com",
  "_user@example.com",
  "www.user@example.com",
];

// What each trigger links to when the preceding character lets it through, so a
// row below reads as a pattern instead of four repeated URLs.
const W = "http://www.example.com";
const H = "http://example.com";
const E = "mailto:user@example.com";
const U = "mailto:_user@example.com";
// The last trigger is a `www.` literal and an email at the same offset. The
// email is registered first, so it wins wherever the preceding character lets
// it through — and only there does the `www.` half get the match.
const O = "mailto:www.user@example.com";
const OW = "http://www.user@example.com";
/** The trigger does not become a link at all. */
const NO = "";

const PRECEDING: Array<{ prefix: string; name: string; urls: string[] }> = [
  { prefix: "", name: "start of document", urls: [W, H, E, U, O] },
  { prefix: " ", name: "space", urls: [W, H, E, U, O] },
  { prefix: "  ", name: "two spaces", urls: [W, H, E, U, O] },
  { prefix: "(", name: "`(`", urls: [W, H, E, U, O] },
  { prefix: "*", name: "`*`", urls: [W, H, E, U, O] },
  {
    prefix: "_",
    name: "`_`",
    urls: [W, H, U, "mailto:__user@example.com", "mailto:_www.user@example.com"],
  },
  { prefix: "~", name: "`~`", urls: [W, H, E, U, O] },
  { prefix: "]", name: "`]`", urls: [W, H, E, U, O] },
  { prefix: ">", name: "`>` (a blockquote marker here)", urls: [W, H, E, U, O] },
  {
    prefix: ".",
    name: "`.`",
    urls: [
      W,
      H,
      "mailto:.user@example.com",
      "mailto:._user@example.com",
      "mailto:.www.user@example.com",
    ],
  },
  { prefix: ",", name: "`,`", urls: [W, H, E, U, O] },
  { prefix: '"', name: '`"`', urls: [W, H, E, U, O] },
  {
    prefix: "-",
    name: "`-`",
    urls: [
      W,
      H,
      "mailto:-user@example.com",
      "mailto:-_user@example.com",
      "mailto:-www.user@example.com",
    ],
  },
  { prefix: "|", name: "`|`", urls: [W, H, E, U, O] },
  { prefix: "/", name: "`/`", urls: [W, H, NO, E, OW] },
  { prefix: "[", name: "`[`", urls: [W, H, E, U, OW] },
  { prefix: "\\", name: "`\\`", urls: [W, H, E, U, O] },
  { prefix: "©", name: "So symbol", urls: [W, H, E, U, O] },
  { prefix: "€", name: "Sc symbol", urls: [W, H, E, U, O] },
  { prefix: "±", name: "Sm symbol", urls: [W, H, E, U, O] },
  { prefix: "—", name: "Pd punctuation", urls: [W, H, E, U, O] },
  { prefix: "•", name: "Po punctuation", urls: [W, H, E, U, O] },
  { prefix: "。", name: "CJK Po punctuation", urls: [W, H, E, U, O] },
  { prefix: "（", name: "Ps punctuation", urls: [W, H, E, U, O] },
  { prefix: "\u{a0}", name: "Zs space", urls: [W, H, E, U, O] },
  {
    prefix: "5",
    name: "Nd digit",
    urls: [
      NO,
      H,
      "mailto:5user@example.com",
      "mailto:5_user@example.com",
      "mailto:5www.user@example.com",
    ],
  },
  {
    prefix: "a",
    name: "ASCII letter",
    urls: [
      NO,
      NO,
      "mailto:auser@example.com",
      "mailto:a_user@example.com",
      "mailto:awww.user@example.com",
    ],
  },
  { prefix: "α", name: "Ll letter", urls: [NO, H, E, U, O] },
  { prefix: "中", name: "Lo letter", urls: [NO, H, E, U, O] },
  { prefix: "e\u{301}", name: "Mn combining mark", urls: [NO, H, E, U, O] },
  { prefix: "\u{200b}", name: "Cf zero-width space", urls: [NO, H, E, U, O] },
  { prefix: "\u{ad}", name: "Cf soft hyphen", urls: [NO, H, E, U, O] },
  { prefix: "❤\u{fe0f}", name: "variation selector", urls: [NO, H, E, U, O] },
  { prefix: "🯰", name: "astral Nd digit", urls: [NO, H, E, U, O] },
  { prefix: "𝐀", name: "astral Lu letter", urls: [NO, H, E, U, O] },
  { prefix: "𠀀", name: "astral Lo letter", urls: [NO, H, E, U, O] },
];

describe("family A: the preceding-character classifier", () => {
  test.each(PRECEDING)("$name", ({ prefix, urls }) => {
    for (const [ix, trigger] of A_TRIGGERS.entries()) {
      const md = `${prefix}${trigger}\n`;
      assertMdastConformance(md);
      expect(linkUrls(md), JSON.stringify(md)).toEqual(urls[ix] === "" ? [] : [urls[ix]]);
    }
  });

  // The trigger kinds the table above does not carry.
  test.each([
    ["www.example.com/a/b\n", ["http://www.example.com/a/b"]],
    ["https://example.com/a?b=c#d\n", ["https://example.com/a?b=c#d"]],
    ["HtTp://Example.COM\n", ["HtTp://Example.COM"]],
    ["u-s.e_r@sub.example.co.uk\n", ["mailto:u-s.e_r@sub.example.co.uk"]],
    ["mailto:user@example.com\n", ["mailto:user@example.com"]],
    ["xmpp:user@example.com\n", ["mailto:user@example.com"]],
    ["(https://example.com/a?b=c#d\n", ["https://example.com/a?b=c#d"]],
    ["(mailto:user@example.com\n", ["mailto:user@example.com"]],
    [".mailto:user@example.com\n", ["mailto:user@example.com"]],
    [".xmpp:user@example.com\n", ["mailto:user@example.com"]],
  ])("%j", conforms);

  // Mid-line, the same prefixes classify the same way — except these two, where
  // the character means something else at the start of a line.
  test.each([
    ["x \twww.example.com\n", ["http://www.example.com"]],
    ["x \tuser@example.com\n", ["mailto:user@example.com"]],
    ["x \thttp://example.com\n", ["http://example.com"]],
    ["x >www.example.com\n", ["http://www.example.com"]],
    ["x >user@example.com\n", ["mailto:user@example.com"]],
    ["x >http://example.com\n", ["http://example.com"]],
  ])("%j", conforms);
});

// Astral characters before a trigger are a documented divergence, pinned in
// autolink-path.test.ts alongside remark's side of it.

// Family B — the trailing-punctuation and trailing-entity rules. For `www.` and
// `http://` the trim is a set-membership test, so there is one row per member: a
// member quietly leaving the trim set (or joining it) fails exactly one row here
// and nothing else. Trailing runs that are not a member and only repeat "left
// alone" are dropped.
//
// Emails get no trim at all — `fnr_find_email` reports the scan's end as the URL
// end — so the third block is not a member sweep: the scan simply stops at the
// first byte the domain rule rejects, and the rows are the few characters the
// rule does read (`.`, `_`, `-`).
describe("family B: trailing punctuation and entities", () => {
  test.each([
    ["www.example.com\n", ["http://www.example.com"]],
    ["www.example.com.\n", ["http://www.example.com"]],
    ["www.example.com...\n", ["http://www.example.com"]],
    ["www.example.com,\n", ["http://www.example.com"]],
    ["www.example.com;\n", ["http://www.example.com"]],
    ["www.example.com:\n", ["http://www.example.com"]],
    ["www.example.com!\n", ["http://www.example.com"]],
    ["www.example.com?!\n", ["http://www.example.com"]],
    ["www.example.com'\n", ["http://www.example.com"]],
    ['www.example.com"\n', ["http://www.example.com"]],
    ["www.example.com*\n", ["http://www.example.com"]],
    ["www.example.com**\n", ["http://www.example.com"]],
    ["www.example.com_\n", ["http://www.example.com"]],
    ["www.example.com~~\n", ["http://www.example.com"]],
    ["www.example.com)\n", ["http://www.example.com"]],
    ["www.example.com))\n", ["http://www.example.com"]],
    ["www.example.com(\n", ["http://www.example.com("]],
    ["www.example.com()\n", ["http://www.example.com()"]],
    ["www.example.com]\n", ["http://www.example.com"]],
    ["www.example.com<\n", ["http://www.example.com"]],
    ["www.example.com|\n", ["http://www.example.com|"]],
    ["www.example.com&amp;.\n", ["http://www.example.com"]],
    ["www.example.com&copy;\n", ["http://www.example.com"]],
    ["www.example.com&#35;\n", ["http://www.example.com&#35"]],
    ["www.example.com&#x23;\n", ["http://www.example.com&#x23"]],
    ["www.example.com&notreal;\n", ["http://www.example.com"]],
    ["www.example.com&;\n", ["http://www.example.com&"]],
    ["www.example.com&amp\n", ["http://www.example.com&amp"]],
    ["www.example.com&amp;amp;\n", ["http://www.example.com&amp;amp"]],
    ["www.example.com&nbsp;\n", ["http://www.example.com"]],
    ["www.example.com&lt;\n", ["http://www.example.com"]],
    ["www.example.com&#0;\n", ["http://www.example.com&#0"]],
    ["www.example.com&#xFFFF;\n", ["http://www.example.com&#xFFFF"]],
    ["www.example.com&#1114112;\n", ["http://www.example.com&#1114112"]],
  ])("%j", conforms);

  test.each([
    ["www.example.com/p\n", ["http://www.example.com/p"]],
    ["www.example.com/p.\n", ["http://www.example.com/p"]],
    ["www.example.com/p-\n", ["http://www.example.com/p-"]],
    ["www.example.com/p_\n", ["http://www.example.com/p"]],
    ["www.example.com/p\\\n", ["http://www.example.com/p\\"]],
    ["www.example.com/p(\n", ["http://www.example.com/p("]],
    ["www.example.com/p()\n", ["http://www.example.com/p()"]],
    ["www.example.com/p)\n", ["http://www.example.com/p"]],
    ["www.example.com/p))\n", ["http://www.example.com/p"]],
    ["www.example.com/p&amp;\n", ["http://www.example.com/p"]],
    ["www.example.com/p&amp;.\n", ["http://www.example.com/p"]],
    ["www.example.com/p&amp;)\n", ["http://www.example.com/p"]],
  ])("%j", conforms);

  test.each([
    ["user@example.com\n", ["mailto:user@example.com"]],
    ["user@example.com.\n", ["mailto:user@example.com"]],
    ["user@example.com...\n", ["mailto:user@example.com"]],
    ["user@example.com_\n", []],
    ["user@example.com-\n", []],
  ])("%j", conforms);

  // The same trims with text after them, where the trimmed tail has somewhere
  // to go.
  test.each([
    ["see www.example.com end\n", ["http://www.example.com"]],
    ["see www.example.com. end\n", ["http://www.example.com"]],
    ["see www.example.com&amp; end\n", ["http://www.example.com"]],
    ["see www.example.com( end\n", ["http://www.example.com("]],
    ["see www.example.com< end\n", ["http://www.example.com"]],
    ["see www.example.com\\ end\n", ["http://www.example.com\\"]],
    ["see www.example.com** end\n", ["http://www.example.com"]],
    ["see www.example.com&#1114112; end\n", ["http://www.example.com&#1114112"]],
  ])("%j", conforms);
});

// Family C — the GFM balanced-paren rule.
describe("family C: the balanced-paren rule", () => {
  test.each([
    ["www.example.com/a(b\n", ["http://www.example.com/a(b"]],
    ["www.example.com/a)b\n", ["http://www.example.com/a)b"]],
    ["www.example.com/a(b)c)\n", ["http://www.example.com/a(b)c"]],
    ["www.example.com/a(b(c))\n", ["http://www.example.com/a(b(c))"]],
    ["www.example.com/a(b(c)\n", ["http://www.example.com/a(b(c)"]],
    ["www.example.com/(\n", ["http://www.example.com/("]],
    ["www.example.com/)\n", ["http://www.example.com/"]],
    ["www.example.com/()\n", ["http://www.example.com/()"]],
    ["www.example.com/)(\n", ["http://www.example.com/)("]],
    ["www.example.com/a(b).\n", ["http://www.example.com/a(b)"]],
    ["http://example.com/a(b)c\n", ["http://example.com/a(b)c"]],
    ["http://example.com/foo).\n", ["http://example.com/foo"]],
    ["http://example.com/(a)(b)\n", ["http://example.com/(a)(b)"]],
    ["http://example.com/(a)(b\n", ["http://example.com/(a)(b"]],
    [
      "https://en.wikipedia.org/wiki/Ruby_(programming_language)\n",
      ["https://en.wikipedia.org/wiki/Ruby_(programming_language)"],
    ],
    [
      "https://en.wikipedia.org/wiki/Ruby_(programming_language))\n",
      ["https://en.wikipedia.org/wiki/Ruby_(programming_language)"],
    ],
    [
      "(https://en.wikipedia.org/wiki/Ruby_(programming_language))\n",
      ["https://en.wikipedia.org/wiki/Ruby_(programming_language)"],
    ],
    ["(www.example.com/a)\n", ["http://www.example.com/a"]],
    ["((www.example.com))\n", ["http://www.example.com"]],
    ["(www.example.com\n", ["http://www.example.com"]],
    ["x (www.example.com) y\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

// Family G — unicode inside and around the URL, and the rule that an underscore
// may not appear in either of the last two domain labels.
describe("family G: unicode in and around the URL", () => {
  test.each([
    ["www.exämple.com\n", ["http://www.exämple.com"]],
    ["www.example.com/ä\n", ["http://www.example.com/ä"]],
    ["www.例え.com\n", ["http://www.例え.com"]],
    ["www.example.com/😀\n", ["http://www.example.com/😀"]],
    ["https://例え.テスト\n", ["https://例え.テスト"]],
    ["https://example.com/päth?q=ü#frag\n", ["https://example.com/päth?q=ü#frag"]],
    ["ü@example.com\n", []],
    ["user@exämple.com\n", []],
    ["😀 www.example.com\n", ["http://www.example.com"]],
    ["www.example.com😀\n", ["http://www.example.com😀"]],
    ["中www.example.com\n", []],
    ["www.example.com/😀.\n", ["http://www.example.com/😀"]],
    ["www.example.com/a—b\n", ["http://www.example.com/a—b"]],
    ["www.example.com—\n", ["http://www.example.com—"]],
    ["www.example.com\u{200b}\n", ["http://www.example.com\u{200b}"]],
    ["www.example.com\u{a0}x\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("family G: underscores in the last two domain labels", () => {
  test.each([
    ["www.exa_mple.com\n", []],
    ["www.example_.com\n", []],
    ["www.example.c_om\n", []],
    ["www.a.exa_mple.com\n", []],
    ["www.a_b.example.com\n", ["http://www.a_b.example.com"]],
    ["www.a_b.c_d.example.com\n", ["http://www.a_b.c_d.example.com"]],
    ["http://exa_mple.com\n", []],
    ["http://a.exa_mple.com\n", []],
    ["http://a_b.c.example.com\n", ["http://a_b.c.example.com"]],
    ["http://foo_bar.com\n", []],
    ["http://foo_bar.com.\n", ["http://foo_bar.com"]],
    ["http://a.b_c\n", []],
    ["http://a_b\n", []],
    ["www.a_b\n", []],
    ["user@exa_mple.com\n", ["mailto:user@exa_mple.com"]],
    ["user@a_b.example.com\n", ["mailto:user@a_b.example.com"]],
    ["www.example.com/a_b\n", ["http://www.example.com/a_b"]],
    ["www.example.com?a_b\n", ["http://www.example.com?a_b"]],
    ["www.example.com__\n", ["http://www.example.com"]],
    ["www.example.com_x\n", []],
  ])("%j", conforms);
});

// Family H — one case per clause of GFM §6.9 (www / url / email autolink
// extended). The spec's own examples already run as HTML in the generated
// `gfm_autolink` suite on the Rust side; the tree and the URL are a different
// assertion layer, which is why they are repeated here. The enumerations around
// them are cut to boundary members: a character outside `is_email_local_char`
// only repeats "the backward walk stops here", whichever character it is.
describe("family H: GFM §6.9 spec clauses", () => {
  test.each([
    ["www.commonmark.org\n", ["http://www.commonmark.org"]],
    ["Visit www.commonmark.org/help for more information.\n", ["http://www.commonmark.org/help"]],
    [
      "Visit www.commonmark.org.\n\nVisit www.commonmark.org/a.b.\n",
      ["http://www.commonmark.org", "http://www.commonmark.org/a.b"],
    ],
    [
      "www.google.com/search?q=Markup+(business)\n",
      ["http://www.google.com/search?q=Markup+(business)"],
    ],
    [
      "www.google.com/search?q=Markup+(business))\n",
      ["http://www.google.com/search?q=Markup+(business)"],
    ],
    [
      "(www.google.com/search?q=Markup+(business))\n",
      ["http://www.google.com/search?q=Markup+(business)"],
    ],
    [
      "(www.google.com/search?q=Markup+(business)\n",
      ["http://www.google.com/search?q=Markup+(business)"],
    ],
    ["www.google.com/search?q=(business))+ok\n", ["http://www.google.com/search?q=(business))+ok"]],
    [
      "www.google.com/search?q=commonmark&hl=en\n",
      ["http://www.google.com/search?q=commonmark&hl=en"],
    ],
    ["www.google.com/search?q=commonmark&hl;\n", ["http://www.google.com/search?q=commonmark"]],
    ["www.commonmark.org/he<lp\n", ["http://www.commonmark.org/he"]],
    ["http://commonmark.org\n", ["http://commonmark.org"]],
    [
      "(Visit https://encrypted.google.com/search?q=Markup+(business))\n",
      ["https://encrypted.google.com/search?q=Markup+(business)"],
    ],
    ["Anonymous FTP is available at ftp://foo.bar.baz.\n", []],
    ["foo@bar.baz\n", ["mailto:foo@bar.baz"]],
    [
      "hello@mail+xyz.example isn't valid, but hello+xyz@mail.example is.\n",
      ["mailto:hello+xyz@mail.example"],
    ],
    ["a.b-c_d@a.b\n", ["mailto:a.b-c_d@a.b"]],
    ["a.b-c_d@a.b.\n", ["mailto:a.b-c_d@a.b"]],
    ["a.b-c_d@a.b-\n", []],
    ["a.b-c_d@a.b_\n", []],
    ["user@localhost\n", []],
    ["http://a\n", ["http://a"]],
    ["http://.\n", []],
    ["http://.com\n", ["http://.com"]],
    ["www..com\n", ["http://www..com"]],
    ["http:/example.com\n", []],
    ["http:example.com\n", []],
    ["://example.com\n", []],
    ["@example.com\n", []],
    ["user@\n", []],
    ["user@.com\n", ["mailto:user@.com"]],
    ["user@-.com\n", ["mailto:user@-.com"]],
    ["user@a..b\n", []],
    ["user@a.b..\n", ["mailto:user@a.b"]],
    ["www.-example.com\n", ["http://www.-example.com"]],
    ["www.example-.com\n", ["http://www.example-.com"]],
    ["www.exam-ple.com\n", ["http://www.exam-ple.com"]],
    ["http://a-b.c-d.example.com\n", ["http://a-b.c-d.example.com"]],
    ["HTTPS://example.com\n", ["HTTPS://example.com"]],
    ["WWW.example.com\n", ["http://WWW.example.com"]],
    ["5www.example.com\n", []],
    ["awww.example.com\n", []],
    ["a.www.example.com\n", ["http://www.example.com"]],
    ["xhttp://example.com\n", []],
    ["5http://example.com\n", ["http://example.com"]],
    [".http://example.com\n", ["http://example.com"]],
    ["a+b@c.com\n", ["mailto:a+b@c.com"]],
    ["a-b@c.com\n", ["mailto:a-b@c.com"]],
    ["a.b@c.com\n", ["mailto:a.b@c.com"]],
    ["a_b@c.com\n", ["mailto:a_b@c.com"]],
    ["a!b@c.com\n", ["mailto:b@c.com"]],
    ["a@b@c.com\n", ["mailto:b@c.com"]],
    [".a@b.com\n", ["mailto:.a@b.com"]],
    ["user@example.co-m\n", ["mailto:user@example.co-m"]],
    ["user@example.co_m\n", ["mailto:user@example.co_m"]],
    ["http://example.com/a<b\n", ["http://example.com/a"]],
    ["user@example.com<x\n", ["mailto:user@example.com"]],
    ["www.example.com/?a=1&b=2;\n", ["http://www.example.com/?a=1&b=2"]],
    ["www.example.com&amp;;\n", ["http://www.example.com"]],
    ["www.example.com&x;\n", ["http://www.example.com"]],
    ["www.example.com&#;\n", ["http://www.example.com&#"]],
    ["www.example.com/a&amp;b\n", ["http://www.example.com/a&amp;b"]],
    ["http://example.com?a&copy;\n", ["http://example.com?a"]],
  ])("%j", conforms);
});

// Family J — unicode whitespace. link-edge-cases.test.ts already covers it
// inside a www/http URL body; these are the shapes it does not reach: the email
// forms, the preceding-character (boundary) forms, and the find-and-replace
// path. U+0085 is the boundary the `www` classifier had to be taught, and the two
// `Cf` code points are the controls that must stay inside the URL.
//
// One code point per branch of `is_autolink_whitespace`: U+00A0 stands for every
// code point that reaches it through `char::is_whitespace()`.
describe("family J: unicode whitespace as terminator and boundary", () => {
  test.each([
    ["user@example.com\u{85}x\n", ["mailto:user@example.com"]],
    ["user@exa\u{85}mple.com\n", []],
    ["https://example.com\u{85}\n", ["https://example.com\u{85}"]],
    ["x\u{85}www.example.com\n", []],
    ["x\u{85}http://example.com\n", ["http://example.com"]],
    ["x\u{85}user@example.com\n", ["mailto:user@example.com"]],
    ["x\u{85}_user@example.com\n", ["mailto:_user@example.com"]],
    ["[a www.example.com/p\u{85}q\n", ["http://www.example.com/p\u{85}q"]],
    ["[a x\u{85}www.example.com\n", []],
    ["user@example.com\u{a0}x\n", ["mailto:user@example.com"]],
    ["user@exa\u{a0}mple.com\n", []],
    ["https://example.com\u{a0}\n", ["https://example.com"]],
    ["x\u{a0}www.example.com\n", ["http://www.example.com"]],
    ["x\u{a0}http://example.com\n", ["http://example.com"]],
    ["x\u{a0}user@example.com\n", ["mailto:user@example.com"]],
    ["x\u{a0}_user@example.com\n", ["mailto:_user@example.com"]],
    ["[a www.example.com/p\u{a0}q\n", ["http://www.example.com/p\u{a0}q"]],
    ["[a x\u{a0}www.example.com\n", ["http://www.example.com"]],
    ["user@example.com\u{200b}x\n", ["mailto:user@example.com"]],
    ["user@exa\u{200b}mple.com\n", []],
    ["https://example.com\u{200b}\n", ["https://example.com\u{200b}"]],
    ["x\u{200b}www.example.com\n", []],
    ["x\u{200b}http://example.com\n", ["http://example.com"]],
    ["x\u{200b}user@example.com\n", ["mailto:user@example.com"]],
    ["x\u{200b}_user@example.com\n", ["mailto:_user@example.com"]],
    ["[a www.example.com/p\u{200b}q\n", ["http://www.example.com/p\u{200b}q"]],
    ["[a x\u{200b}www.example.com\n", []],
    // U+FEFF has no `https://example.com{WS}` or `user@example.com{WS}x` row:
    // both trip the drop pinned as `test.fails` in link-edge-cases.test.ts.
    ["user@exa\u{feff}mple.com\n", []],
    ["x\u{feff}www.example.com\n", ["http://www.example.com"]],
    ["x\u{feff}http://example.com\n", ["http://example.com"]],
    ["x\u{feff}user@example.com\n", ["mailto:user@example.com"]],
    ["x\u{feff}_user@example.com\n", ["mailto:_user@example.com"]],
    ["[a www.example.com/p\u{feff}q\n", ["http://www.example.com/p\u{feff}q"]],
    ["[a x\u{feff}www.example.com\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

// GFM registers the email construct ahead of `www` at the same offset, so an
// email whose local part opens with `www.` beats the www literal that could
// start there. The rows below are the whole family: every failing shape the
// differential sweep found was one of these local parts under one of the
// preceding characters `www` itself accepts.
describe("family M: email and `www` triggering at the same offset", () => {
  test.each([
    ["www.x.ya@b.cd\n", ["mailto:www.x.ya@b.cd"]],
    // The extent shrinks too, not just the scheme: `/p` stays text.
    ["www.x.ya@b.cd/p\n", ["mailto:www.x.ya@b.cd"]],
    ["WWW.x@b.cd\n", ["mailto:WWW.x@b.cd"]],
    ["wWw.x@b.cd\n", ["mailto:wWw.x@b.cd"]],
    ["www.@b.cd\n", ["mailto:www.@b.cd"]],
    ["www.-@b.cd\n", ["mailto:www.-@b.cd"]],
    ["www.1@b.cd\n", ["mailto:www.1@b.cd"]],
    ["www.a.b.c@d.ef\n", ["mailto:www.a.b.c@d.ef"]],
    // The email construct fails on its own terms here, so `www` still wins.
    ["www.x.ya@b\n", ["http://www.x.ya@b"]],
    // `_` is trailing punctuation, so the www URL trims it — and the email
    // domain it would have ended on is rejected for not ending alphabetic.
    ["www.x.ya@b.cd_\n", ["http://www.x.ya@b.cd"]],
    // The domain scan rejects outright, so no span is skipped and the `@`
    // hook picks it up unaided.
    ["www.a_b@c.de\n", ["mailto:www.a_b@c.de"]],
    ["_www.x.ya@b.cd\n", ["mailto:_www.x.ya@b.cd"]],
    // An atext run from `h` stops at `:`, so a protocol literal and an email
    // can never open at the same offset.
    ["http://x.ya@b.cd\n", ["http://x.ya@b.cd"]],
    ["https://x.ya@b.cd\n", ["https://x.ya@b.cd"]],
    // The characters `www` accepts as a predecessor.
    ["(www.x.ya@b.cd)\n", ["mailto:www.x.ya@b.cd"]],
    ["*www.x.ya@b.cd*\n", ["mailto:www.x.ya@b.cd"]],
    ["~www.x.ya@b.cd~\n", ["mailto:www.x.ya@b.cd"]],
    ["]www.x.ya@b.cd\n", ["mailto:www.x.ya@b.cd"]],
    ["x www.x.ya@b.cd\n", ["mailto:www.x.ya@b.cd"]],
    // Not predecessors `www` accepts, so only the `@` hook can fire.
    ["a.www.x.ya@b.cd\n", ["mailto:a.www.x.ya@b.cd"]],
    ["1www.x.ya@b.cd\n", ["mailto:1www.x.ya@b.cd"]],
    // The email ends before the www literal would have, and the bytes between
    // the two ends go back through inline scanning as ordinary content.
    ["www.x.ya@b.cd*em*\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd\\*\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd&amp;\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd)\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd<b>\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd`c`\n", ["mailto:www.x.ya@b.cd"]],
  ])("%j", conforms);
});
