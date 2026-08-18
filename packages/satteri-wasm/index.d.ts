export {
  initSync,
  default as init,
  markdown_to_html as markdownToHtml,
  mdx_to_hast as mdxToHastBuffer,
  mdx_to_js as mdxToJs,
} from "./dist/satteri_wasm";
export type { InitInput, InitOutput, SyncInitInput } from "./dist/satteri_wasm";
export type HastRoot = ReturnType<typeof import("satteri/hast").materializeHastTree>;

export function mdxToHast(source: string): HastRoot;
