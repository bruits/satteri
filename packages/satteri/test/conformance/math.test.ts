import { describe, test } from "vitest";
import {
  assertExtMdastConformance,
  assertExtHastConformance,
  assertNoSingleDollarMathMdastConformance,
  assertNoSingleDollarMathHastConformance,
} from "./helpers.js";

const MATH: ["math"] = ["math"];

describe("Math MDAST conformance", () => {
  test("basic inline and display math", () => {
    assertExtMdastConformance("Math $\\alpha$\n\n$$\n\\beta+\\gamma\n$$", MATH);
  });

  test("escaped opening dollar sign", () => {
    assertExtMdastConformance("\\$\\alpha$", MATH);
  });

  test("escaped closing dollar sign", () => {
    assertExtMdastConformance("$\\alpha\\$", MATH);
  });

  test("escaped escape before dollar sign", () => {
    assertExtMdastConformance("\\\\\\$\\alpha$", MATH);
  });

  test("dollar signs in inline code", () => {
    assertExtMdastConformance("`$`\\alpha$", MATH);
  });

  test("backticks in math", () => {
    assertExtMdastConformance("$\\alpha`$`", MATH);
  });

  test("double dollar inline", () => {
    assertExtMdastConformance("$$\\alpha$$", MATH);
  });

  test("display math after paragraph", () => {
    assertExtMdastConformance("tango\n$$\n\\alpha\n$$", MATH);
  });

  test("triple dollar display math", () => {
    assertExtMdastConformance("$$$\n\\alpha\n$$$", MATH);
  });

  test("indented display math", () => {
    assertExtMdastConformance("  $$\n    \\alpha\n  $$", MATH);
  });

  test("opening fence without closing", () => {
    assertExtMdastConformance("$$just two dollars", MATH);
  });

  test("meta text after opening fence", () => {
    assertExtMdastConformance("$$  must\n\\alpha\n$$", MATH);
  });

  test("values before closing fence", () => {
    assertExtMdastConformance("$$\n\\alpha\nmust  $$", MATH);
  });

  test("spacing before closing fence", () => {
    assertExtMdastConformance("$$\n\\alpha\n  $$", MATH);
  });

  test("spacing after closing fence", () => {
    assertExtMdastConformance("$$\n\\alpha\n$$  ", MATH);
  });

  test("display math followed by code block", () => {
    assertExtMdastConformance("$$\n\\alpha\n$$\n```\nbravo\n```", MATH);
  });

  test("basic arithmetic", () => {
    assertExtMdastConformance("$1+1 = 2$", MATH);
  });

  test("math without surrounding whitespace", () => {
    assertExtMdastConformance("foo$1+1 = 2$bar", MATH);
  });

  test("math starting with negative sign", () => {
    assertExtMdastConformance("foo$-1+1 = 2$bar", MATH);
  });

  test("empty math content", () => {
    assertExtMdastConformance("aaa $$ bbb", MATH);
  });

  test("unclosed delimiter", () => {
    assertExtMdastConformance("aaa $5.99 bbb", MATH);
  });

  test("paragraph break in inline math", () => {
    assertExtMdastConformance("foo $1+1\n\n= 2$ bar", MATH);
  });

  test("markup inside math", () => {
    assertExtMdastConformance("foo $1 *i* 1$ bar", MATH);
  });

  test("3-space indented display math", () => {
    assertExtMdastConformance("   $$\n   1+1 = 2\n   $$", MATH);
  });

  test("4-space indentation becomes code block", () => {
    assertExtMdastConformance("    $$\n    1+1 = 2\n    $$", MATH);
  });

  test("multiline inline math", () => {
    assertExtMdastConformance("foo $1 + 1\n= 2$ bar", MATH);
  });

  test("multiline display math", () => {
    assertExtMdastConformance("$$\n\n  1\n+ 1\n\n= 2\n\n$$", MATH);
  });

  test("text following inline math", () => {
    assertExtMdastConformance("$n$-th order", MATH);
  });

  test("display math at end of document", () => {
    assertExtMdastConformance("$$\n1+1 = 2", MATH);
  });

  test("math in lists", () => {
    assertExtMdastConformance("* $1+1 = 2$\n* $$\n  1+1 = 2\n  $$", MATH);
  });

  test("escaped delimiters not parsed", () => {
    assertExtMdastConformance("Foo \\$1$ bar\n\n\\$\\$\n1\n\\$\\$", MATH);
  });

  test("currency amounts not parsed", () => {
    assertExtMdastConformance("Thus, $20,000 and USD$30,000 won't parse", MATH);
  });

  test("whitespace after opening dollar", () => {
    assertExtMdastConformance("It is 2$ for a can of soda, not 1$.", MATH);
  });

  test("whitespace before closing dollar", () => {
    assertExtMdastConformance("I'll give $20 today, if you give me more $ tomorrow.", MATH);
  });

  test("escaped dollars inside math", () => {
    assertExtMdastConformance("Money adds: $\\$X + \\$Y = \\$Z$.", MATH);
  });

  test("double dollar inline with spaces stripped", () => {
    assertExtMdastConformance("$$ Display `first $$ then` code", MATH);
  });

  test("double dollar inline with leading and trailing spaces", () => {
    assertExtMdastConformance("$$ x $$", MATH);
  });

  test("single dollar with spaces stripped", () => {
    assertExtMdastConformance("$ x $", MATH);
  });

  test("spaces not stripped when content is only spaces", () => {
    assertExtMdastConformance("$ $", MATH);
  });

  test("double spaces not stripped when content is only spaces", () => {
    assertExtMdastConformance("$$  $$", MATH);
  });

  test("extra spaces partially stripped", () => {
    assertExtMdastConformance("$$  x  $$", MATH);
  });

  test("many consecutive dollars no match", () => {
    assertExtMdastConformance("$x$$$$$$$y$$", MATH);
  });

  test("six consecutive dollars no match", () => {
    assertExtMdastConformance("$x$$$$$$y$$", MATH);
  });

  test("triple dollar matching triple dollar", () => {
    assertExtMdastConformance("$$$x$$$", MATH);
  });

  test("triple dollar with spaces", () => {
    assertExtMdastConformance("$$$ x $$$", MATH);
  });

  test("mismatched triple vs double", () => {
    assertExtMdastConformance("$$$x$$", MATH);
  });

  test("mismatched double vs triple", () => {
    assertExtMdastConformance("$$x$$$", MATH);
  });

  test("double dollar inline multiline with spaces", () => {
    assertExtMdastConformance(
      "When $a \\ne 0$, there are two solutions to $(ax^2 + bx + c = 0)$ and they are\n$$ x = {-b \\pm \\sqrt{b^2-4ac} \\over 2a} $$",
      MATH,
    );
  });

  test("empty double dollar inline", () => {
    assertExtMdastConformance("Oops empty $$ expression.", MATH);
  });

  test("code first then double dollar", () => {
    assertExtMdastConformance("`Code $$ first` then $$ display", MATH);
  });
});

describe("Math HAST conformance", () => {
  test("basic inline math", () => {
    assertExtHastConformance("$\\alpha$", MATH);
  });

  test("basic display math", () => {
    assertExtHastConformance("$$\n\\beta+\\gamma\n$$", MATH);
  });

  test("inline and display together", () => {
    assertExtHastConformance("Math $\\alpha$\n\n$$\n\\beta+\\gamma\n$$", MATH);
  });

  test("basic arithmetic", () => {
    assertExtHastConformance("$1+1 = 2$", MATH);
  });

  test("math without surrounding whitespace", () => {
    assertExtHastConformance("foo$1+1 = 2$bar", MATH);
  });

  test("text following inline math", () => {
    assertExtHastConformance("$n$-th order", MATH);
  });

  test("display math after paragraph", () => {
    assertExtHastConformance("tango\n$$\n\\alpha\n$$", MATH);
  });

  test("double dollar inline", () => {
    assertExtHastConformance("$$\\alpha$$", MATH);
  });

  test("currency amounts not parsed", () => {
    assertExtHastConformance("Thus, $20,000 and USD$30,000 won't parse", MATH);
  });

  test("math in lists", () => {
    assertExtHastConformance("* $1+1 = 2$\n* $$\n  1+1 = 2\n  $$", MATH);
  });

  test("display math at end of document", () => {
    assertExtHastConformance("$$\n1+1 = 2", MATH);
  });

  test("multiline display math", () => {
    assertExtHastConformance("$$\n\n  1\n+ 1\n\n= 2\n\n$$", MATH);
  });

  test("escaped delimiters not parsed", () => {
    assertExtHastConformance("Foo \\$1$ bar\n\n\\$\\$\n1\n\\$\\$", MATH);
  });

  test("escaped dollars inside math", () => {
    assertExtHastConformance("Money adds: $\\$X + \\$Y = \\$Z$.", MATH);
  });

  test("double dollar inline with spaces stripped", () => {
    assertExtHastConformance("$$ Display `first $$ then` code", MATH);
  });

  test("double dollar inline with leading and trailing spaces", () => {
    assertExtHastConformance("$$ x $$", MATH);
  });

  test("single dollar with spaces stripped", () => {
    assertExtHastConformance("$ x $", MATH);
  });

  test("many consecutive dollars no match", () => {
    assertExtHastConformance("$x$$$$$$$y$$", MATH);
  });

  test("triple dollar matching triple dollar", () => {
    assertExtHastConformance("$$$x$$$", MATH);
  });

  test("mismatched triple vs double", () => {
    assertExtHastConformance("$$$x$$", MATH);
  });

  test("double dollar inline multiline with spaces", () => {
    assertExtHastConformance(
      "When $a \\ne 0$, there are two solutions to $(ax^2 + bx + c = 0)$ and they are\n$$ x = {-b \\pm \\sqrt{b^2-4ac} \\over 2a} $$",
      MATH,
    );
  });
});

// Verified against remark-math configured the same way so future drift on
// either side surfaces here.
describe("Math singleDollarTextMath:false conformance (vs remark-math)", () => {
  test("currency prose keeps dollars literal", () => {
    assertNoSingleDollarMathMdastConformance("the deficit grew from $50 to $100 billion");
  });

  test("lone $ in prose is not math", () => {
    assertNoSingleDollarMathMdastConformance("price is $9.99 today");
  });

  test("paired single $ no longer opens math", () => {
    assertNoSingleDollarMathMdastConformance("text $x = 1$ here");
  });

  test("double-dollar inline still parses", () => {
    assertNoSingleDollarMathMdastConformance("text $$x^2$$ end");
  });

  test("block math fence still parses", () => {
    assertNoSingleDollarMathMdastConformance("$$\n\\alpha\n$$");
  });

  test("mixed: paragraph with $ and $$..$$", () => {
    assertNoSingleDollarMathMdastConformance(
      "I owe $5 but the formula is $$E = mc^2$$ regardless.",
    );
  });

  test("escaped dollar still literal", () => {
    assertNoSingleDollarMathMdastConformance("Foo \\$1 bar");
  });

  test("hast: currency prose renders without math elements", () => {
    assertNoSingleDollarMathHastConformance("the deficit grew from $50 to $100 billion");
  });

  test("hast: $$..$$ still becomes display math", () => {
    assertNoSingleDollarMathHastConformance("intro $$\\beta$$ outro");
  });

  test("hast: block math fence", () => {
    assertNoSingleDollarMathHastConformance("$$\n\\gamma\n$$");
  });
});
