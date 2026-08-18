import {
  initSync,
  default as init,
  markdown_to_html as markdownToHtml,
  mdx_to_hast as mdxToHastBuffer,
  mdx_to_js as mdxToJs,
} from "./dist/satteri_wasm.js";
import { compileMdxToHast } from "./materialize.js";

export { init, initSync, markdownToHtml, mdxToHastBuffer, mdxToJs };

export function mdxToHast(source) {
  return compileMdxToHast(source, mdxToHastBuffer);
}
