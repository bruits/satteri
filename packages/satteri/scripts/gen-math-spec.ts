import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkMath from "remark-math";
import remarkRehype from "remark-rehype";
import rehypeStringify from "rehype-stringify";

const processor = unified().use(remarkParse).use(remarkMath).use(remarkRehype).use(rehypeStringify);

function html(md: string): string {
  return processor.processSync(md).toString();
}

interface TestCase {
  description: string;
  input: string;
}

function section(title: string): string {
  return `\n## ${title}\n`;
}

function example(tc: TestCase): string {
  let expected = html(tc.input);
  if (!expected.endsWith("\n")) expected += "\n";
  return `${tc.description}:

\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\` example
${tc.input}
.
${expected}\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`\`
`;
}

const inlineCases: TestCase[] = [
  { description: "Basic inline math", input: "$x=y$" },
  { description: "Inline math with LaTeX", input: "$\\sqrt{3x-1}+(1+x)^2$" },
  { description: "Inline math without surrounding whitespace", input: "foo$1+1 = 2$bar" },
  { description: "Multiple inline math expressions", input: "$a$ and $b$" },
  { description: "Adjacent inline math", input: "$a$$b$" },
  { description: "Multiline inline math", input: "$5x + 2 =\n17$" },
  { description: "Double dollar signs inline produce inline math", input: "$$\\alpha$$" },
  { description: "Math content ignores HTML", input: "$a<b>c</b>$" },
  { description: "Math content ignores backticks", input: "$not `code`$" },
  { description: "Math content ignores markdown links", input: "$![not an](/image)$" },
  { description: "Math content ignores autolinks", input: "$<https://not.a.link/>$" },
  { description: "Math content ignores entities", input: "$&alpha;$" },
  { description: "Inline math cannot start with whitespace", input: "$ y=x$" },
  { description: "Inline math cannot end with whitespace", input: "$y=x $" },
  { description: "Unclosed dollar signs are plain text", input: "Hello $world." },
  { description: "Dollar at end of line", input: "Dollar at end of line$" },
  { description: "Escaped dollar in math", input: "$\\$$" },
  { description: "Backslashes in math", input: "$\\{a\\,b\\}$" },
  { description: "Inline math in emphasis", input: "_$a$ equals $b$_" },
  { description: "Inline math in strong", input: "**$a$ equals $b$**" },
  { description: "Inline math preceded by text", input: "a$x$" },
  { description: "Math as only item on a line", input: "$x=y$" },
  { description: "Text following inline math", input: "$n$-th order" },
  { description: "Math with angle brackets", input: "$a <b > c$" },
  { description: "Math with markdown-like content", input: "$[(a+b)c](d+e)$" },
  { description: "Math with underscores", input: "${a}_b c_{d}$" },
  {
    description: "Currency amounts are not math",
    input: "Thus, $20,000 and USD$30,000 won't parse",
  },
  {
    description: "Whitespace after opening dollar prevents math",
    input: "It is 2$ for a can of soda, not 1$.",
  },
  {
    description: "Whitespace before closing dollar prevents math",
    input: "I'll give $20 today, if you give me more $ tomorrow.",
  },
  { description: "Escaped dollars inside math", input: "Money adds: $\\$X + \\$Y = \\$Z$." },
  { description: "Hard breaks not recognized in math", input: "$not a\\\nhard break  \neither$" },
  { description: "Inline math with escaped closing dollar", input: "$\\alpha\\$" },
];

const precedenceCases: TestCase[] = [
  { description: "Inline math vs code span — math first", input: "$Inline `first$ then` code" },
  { description: "Inline math vs code span — code first", input: "`Code $first` then$ inline" },
  {
    description: "Display math vs code span — math first",
    input: "$$ Display `first $$ then` code",
  },
  {
    description: "Display math vs code span — code first",
    input: "`Code $$ first` then $$ display",
  },
  { description: "Empty inline math not allowed", input: "Oops empty $$ expression." },
  { description: "Greedy left-to-right dollar parsing", input: "$x$$$$$$$y$$" },
  { description: "Greedy left-to-right dollar parsing 2", input: "$x$$$$$$y$$" },
  { description: "Greedy left-to-right dollar parsing 3", input: "$$x$$$$$$y$$" },
  { description: "Alpha then double dollar", input: "alpha$$beta$gamma$$delta" },
];

const blockStructureCases: TestCase[] = [
  {
    description: "Block structure takes precedence — list interrupts math",
    input: "$x + y - z$\n\n$x + y\n- z$\n\n$$ x + y\n> z $$",
  },
  {
    description: "Empty lines start new paragraph, breaking math",
    input: "$not\n\nmath$\n\n$$\nnot\n\nmath\n$$",
  },
  { description: "Nested list structure breaks math", input: "- $not\n    - *\n  math$" },
];

const displayCases: TestCase[] = [
  { description: "Basic display math", input: "$$\n\\beta+\\gamma\n$$" },
  { description: "Display math after paragraph", input: "**Bold**\n\n$$\nx = y\n$$" },
  { description: "Display math with meta string", input: "$$ meta\nx\n$$" },
  { description: "Empty display math", input: "$$\n$$" },
  { description: "Multiple display math blocks", input: "$$\na\n$$\n\n$$\nb\n$$" },
  { description: "Triple dollar fence", input: "$$$\nalpha\n$$$" },
  { description: "Indented display math (up to 3 spaces)", input: "   $$\n   x\n   $$" },
  { description: "4-space indentation becomes code block", input: "    $$\n    x\n    $$" },
  { description: "Unterminated display math at end of document", input: "$$\nunterminated" },
  { description: "Display math in list items", input: "- $a$\n- $$\n  b\n  $$" },
  { description: "Display math interrupts paragraph", input: "tango\n$$\n\\alpha\n$$" },
  { description: "Spacing before closing fence", input: "$$\n\\alpha\n  $$" },
  { description: "Spacing after closing fence", input: "$$\n\\alpha\n$$  " },
  { description: "Display math with blank lines in content", input: "$$\n\n  1\n+ 1\n\n= 2\n\n$$" },
  {
    description: "Escaped delimiters are not math fences",
    input: "Foo \\$1$ bar\n\n\\$\\$\n1\n\\$\\$",
  },
  {
    description: "Display math with Cauchy-Schwarz",
    input:
      "**The Cauchy-Schwarz Inequality**\n\n$$\n\\left( \\sum_{k=1}^n a_k b_k \\right)^2 \\leq \\left( \\sum_{k=1}^n a_k^2 \\right) \\left( \\sum_{k=1}^n b_k^2 \\right)\n$$",
  },
  {
    description: "Display and inline in same list",
    input: "- $a$\n\n  $$\n  a\n  $$\n\n- $$\n  b\n  $$",
  },
  { description: "Display math in blockquote", input: "> $$\n> x\n> $$" },
  { description: "Dollar in display text block", input: "$$\n\\text{$b$}\n$$" },
  {
    description: "Dollar-math with spaces on same line",
    input:
      "When $a \\ne 0$, there are two solutions to $(ax^2 + bx + c = 0)$ and they are\n$$ x = {-b \\pm \\sqrt{b^2-4ac} \\over 2a} $$",
  },
];

const contextCases: TestCase[] = [
  { description: "Math in link", input: "[$a$](x)" },
  {
    description: "Math preceded by various characters",
    input: "$\\pi$\n'$\\pi$\n\"$\\pi$\n($\\pi$\n[$\\pi$\n{$\\pi$\n/$\\pi$",
  },
  { description: "Inline math in italic text", input: "_Equation $\\Omega(69)$ in italic text_" },
  { description: "Inline math wrapped in quotes", input: "$x$ $`y`$" },
  { description: "Math vs HTML mix-up", input: "$a <b > c$\n\n$[(a+b)c](d+e)$\n\n${a}_b c_{d}$" },
  { description: "Spacing around dollar sign in math mode", input: "$x = \\$$" },
  { description: "Math starting with negative sign", input: "foo$-1+1 = 2$bar" },
  {
    description: "Images and math in same list",
    input: "- ![node logo](https://nodejs.org/static/images/logo.svg)\n- $x$",
  },
];

let output = `Run this with \`cargo test --features gen-tests suite::math\`.

# Math

Mathematical expressions following the remark-math convention.
Inline math uses \`$...$\`, display math uses \`$$\` fences on separate lines.
`;

output += section("Inline math");
for (const tc of inlineCases) {
  output += "\n" + example(tc);
}

output += section("Precedence and greedy parsing");
for (const tc of precedenceCases) {
  output += "\n" + example(tc);
}

output += section("Block structure interactions");
for (const tc of blockStructureCases) {
  output += "\n" + example(tc);
}

output += section("Display math");
for (const tc of displayCases) {
  output += "\n" + example(tc);
}

output += section("Math in context");
for (const tc of contextCases) {
  output += "\n" + example(tc);
}

process.stdout.write(output);
