import { render } from "preact";
import postHtml, { frontmatter } from "./post.md";
import Intro from "./intro.mdx";

const fmMount = document.getElementById("markdown-frontmatter");
if (fmMount) {
  fmMount.innerHTML = `<pre style="background:#f4f4f4;padding:0.5rem;border-radius:4px;">frontmatter = ${JSON.stringify(frontmatter, null, 2)}</pre>`;
}

const markdownMount = document.getElementById("markdown-out");
if (markdownMount) {
  markdownMount.innerHTML = postHtml;
}

const mdxMount = document.getElementById("mdx-out");
if (mdxMount) {
  render(<Intro />, mdxMount);
}
