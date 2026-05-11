declare module "*.md" {
  const html: string;
  const frontmatter: Record<string, unknown>;
  export default html;
  export { html, frontmatter };
}

declare module "*.mdx" {
  import type { ComponentType } from "preact";
  const MDXContent: ComponentType<Record<string, unknown>>;
  export const frontmatter: Record<string, unknown>;
  export default MDXContent;
}
