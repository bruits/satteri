import { describe, test, expect } from "vitest";
import { referenceMathSingleDollarOffHtml, satteriMathSingleDollarOffHtml } from "./helpers.js";

// `math: { singleDollar: false }` must match remark-math's
// `singleDollarTextMath: false`: `$$...$$` and `$$` fences stay math, while a
// lone `$` is literal text (so currency prose isn't mis-parsed as math).
function assertParity(md: string): void {
  expect(satteriMathSingleDollarOffHtml(md)).toEqual(referenceMathSingleDollarOffHtml(md));
}

describe("Math with singleDollar: false — remark parity", () => {
  test("lone single-dollar span stays literal", () => {
    assertParity("$x=y$");
  });

  test("two currency amounts on one line stay literal", () => {
    assertParity("$50 to $100 billion");
  });

  test("single unclosed dollar stays literal", () => {
    assertParity("We spent $5.99 today");
  });

  test("single-dollar without whitespace stays literal", () => {
    assertParity("foo$1+1 = 2$bar");
  });

  test("repeated single-dollar spans stay literal", () => {
    assertParity("$a$ and $b$");
  });

  test("double-dollar inline still parses as math", () => {
    assertParity("$$\\alpha$$");
  });

  test("block fence still parses as display math", () => {
    assertParity("$$\n\\beta+\\gamma\n$$");
  });

  test("currency prose and display math coexist", () => {
    assertParity("We raised $5 to $10 million.\n\n$$\nE = mc^2\n$$");
  });

  test("math content with entities matches remark escaping", () => {
    assertParity("$$a<b>c</b>$$");
  });
});
