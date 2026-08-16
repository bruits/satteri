import {
  initSync,
  default as init,
  markdown_to_html as markdownToHtml,
  mdx_to_hast as mdxToHastBuffer,
} from "./dist/satteri_runtime.js";
import { compileMdxToHast } from "./materialize.js";

export { init, initSync, markdownToHtml, mdxToHastBuffer };

export function mdxToHast(source) {
  return compileMdxToHast(source, mdxToHastBuffer);
}
