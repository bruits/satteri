export {
  initSync,
  default as init,
  markdown_to_html as markdownToHtml,
  mdx_to_hast as mdxToHastBuffer,
} from "./dist/satteri_runtime";
export type { InitInput, InitOutput, SyncInitInput } from "./dist/satteri_runtime";
export type HastRoot = ReturnType<typeof import("satteri/hast").materializeHastTree>;

export function mdxToHast(source: string): HastRoot;
