import { describe, test, expect } from "vitest";
import { assertMdastConformance, conforms, linkUrls } from "./helpers.js";

const A_TRIGGERS = [
  "www.example.com",
  "http://example.com",
  "user@example.com",
  "_user@example.com",
  "www.user@example.com",
];

const W = "http://www.example.com";
const H = "http://example.com";
const E = "mailto:user@example.com";
const U = "mailto:_user@example.com";
const O = "mailto:www.user@example.com";
const OW = "http://www.user@example.com";
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

describe("the preceding-character classifier", () => {
  test.each(PRECEDING)("$name", ({ prefix, urls }) => {
    for (const [ix, trigger] of A_TRIGGERS.entries()) {
      const md = `${prefix}${trigger}\n`;
      assertMdastConformance(md);
      expect(linkUrls(md), JSON.stringify(md)).toEqual(urls[ix] === "" ? [] : [urls[ix]]);
    }
  });

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

  test.each([
    ["x \twww.example.com\n", ["http://www.example.com"]],
    ["x \tuser@example.com\n", ["mailto:user@example.com"]],
    ["x \thttp://example.com\n", ["http://example.com"]],
    ["x >www.example.com\n", ["http://www.example.com"]],
    ["x >user@example.com\n", ["mailto:user@example.com"]],
    ["x >http://example.com\n", ["http://example.com"]],
  ])("%j", conforms);
});

describe("trailing punctuation and entities", () => {
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

describe("the balanced-paren rule", () => {
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

describe("unicode in and around the URL", () => {
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

describe("underscores in the last two domain labels", () => {
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

// GFM §6.9: autolinks (extension).
describe("GFM §6.9 spec clauses", () => {
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

describe("unicode whitespace as terminator and boundary", () => {
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
    ["user@exa\u{feff}mple.com\n", []],
    ["x\u{feff}www.example.com\n", ["http://www.example.com"]],
    ["x\u{feff}http://example.com\n", ["http://example.com"]],
    ["x\u{feff}user@example.com\n", ["mailto:user@example.com"]],
    ["x\u{feff}_user@example.com\n", ["mailto:_user@example.com"]],
    ["[a www.example.com/p\u{feff}q\n", ["http://www.example.com/p\u{feff}q"]],
    ["[a x\u{feff}www.example.com\n", ["http://www.example.com"]],
  ])("%j", conforms);
});

describe("email and `www` triggering at the same offset", () => {
  test.each([
    ["www.x.ya@b.cd\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd/p\n", ["mailto:www.x.ya@b.cd"]],
    ["WWW.x@b.cd\n", ["mailto:WWW.x@b.cd"]],
    ["wWw.x@b.cd\n", ["mailto:wWw.x@b.cd"]],
    ["www.@b.cd\n", ["mailto:www.@b.cd"]],
    ["www.-@b.cd\n", ["mailto:www.-@b.cd"]],
    ["www.1@b.cd\n", ["mailto:www.1@b.cd"]],
    ["www.a.b.c@d.ef\n", ["mailto:www.a.b.c@d.ef"]],
    ["www.x.ya@b\n", ["http://www.x.ya@b"]],
    ["www.x.ya@b.cd_\n", ["http://www.x.ya@b.cd"]],
    ["www.a_b@c.de\n", ["mailto:www.a_b@c.de"]],
    ["_www.x.ya@b.cd\n", ["mailto:_www.x.ya@b.cd"]],
    ["http://x.ya@b.cd\n", ["http://x.ya@b.cd"]],
    ["https://x.ya@b.cd\n", ["https://x.ya@b.cd"]],
    ["(www.x.ya@b.cd)\n", ["mailto:www.x.ya@b.cd"]],
    ["*www.x.ya@b.cd*\n", ["mailto:www.x.ya@b.cd"]],
    ["~www.x.ya@b.cd~\n", ["mailto:www.x.ya@b.cd"]],
    ["]www.x.ya@b.cd\n", ["mailto:www.x.ya@b.cd"]],
    ["x www.x.ya@b.cd\n", ["mailto:www.x.ya@b.cd"]],
    ["a.www.x.ya@b.cd\n", ["mailto:a.www.x.ya@b.cd"]],
    ["1www.x.ya@b.cd\n", ["mailto:1www.x.ya@b.cd"]],
    ["www.x.ya@b.cd*em*\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd\\*\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd&amp;\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd)\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd<b>\n", ["mailto:www.x.ya@b.cd"]],
    ["www.x.ya@b.cd`c`\n", ["mailto:www.x.ya@b.cd"]],
  ])("%j", conforms);
});

describe("a www link overlapping an email on the fallback path", () => {
  test.each([
    ["[a user@www.example.org\n", ["http://www.example.org"]],
    ["[a user@www.example.org y\n", ["http://www.example.org"]],
    ["[a user@www.example.org.\n", ["http://www.example.org"]],
    ["[a user@www.x\n", ["http://www.x"]],
    ["[a a\\.b@www.c.com\n", ["http://www.c.com"]],
    ["[a a\\+b@www.c.com\n", ["http://www.c.com"]],
    ["[a x\\+a@www.b.com y\n", ["http://www.b.com"]],
    ["[a /_a@www.b.com y\n", ["http://www.b.com"]],
    ["[a user@http://x.y\n", ["http://x.y"]],
    ["[a a@b.www.x.com\n", ["http://www.x.com"]],
    ["[a a@b.c.www.x.com\n", ["mailto:a@b.c", "http://www.x.com"]],
    ["[a u@b.com www.c.org u2@d.com\n", ["mailto:u@b.com", "http://www.c.org", "mailto:u2@d.com"]],
    ["[a c@www.d.e f@g.hi\n", ["http://www.d.e", "mailto:f@g.hi"]],
    ["[a user@example.com\n", ["mailto:user@example.com"]],
    ["[a user@mail.wwwexample.com\n", ["mailto:user@mail.wwwexample.com"]],
    ["user@www.example.org\n", ["mailto:user@www.example.org"]],
  ])("%j", conforms);
});
