import { pathToFileURL } from "node:url";
import type { Plugin, ResolvedConfig } from "vite";
import { markdownToHtml, mdxToJs } from "satteri";
import type {
  CompileOptions,
  Features,
  Frontmatter,
  HastPluginInput,
  MdastPluginInput,
  MdxCompileOptions,
  MdxOnlyOptions,
} from "satteri";
import { parse as parseYaml } from "yaml";
import { parse as parseToml } from "smol-toml";

/**
 * MDX-specific compile options.
 *
 * Derived from Sätteri's {@link MdxOnlyOptions}, minus `outputFormat` — the
 * Vite plugin always emits an ES module ("program") so Vite/Rollup can
 * import it.
 *
 * `development` is special: when omitted, it's inferred from Vite's command
 * (`serve` → true, `build` → false).
 */
export type MdxOptions = Omit<MdxOnlyOptions, "outputFormat">;

export interface VitePluginSatteriOptions {
  /** Process `.md` files. Default: true. */
  markdown?: boolean;
  /**
   * Process `.mdx` files. Default: true.
   *
   * - `true` / `false` — toggle MDX handling with default compile options.
   * - object — toggle on and configure the MDX compile (`jsxImportSource`,
   *   `optimizeStatic`, …). See `MdxOptions`.
   */
  mdx?: boolean | MdxOptions;

  /** MDAST plugins applied before HAST conversion. Shared across .md and .mdx. */
  mdastPlugins?: MdastPluginInput[];
  /** HAST plugins applied before rendering / MDX compile. Shared across .md and .mdx. */
  hastPlugins?: HastPluginInput[];
  /** Parser feature toggles (gfm, frontmatter, math, …). Shared across .md and .mdx. */
  features?: Features;
}

const MD_RE = /\.md(?:\?|$)/;
const MDX_RE = /\.mdx(?:\?|$)/;

function parseFrontmatter(fm: Frontmatter | null): unknown {
  if (!fm) return {};
  if (fm.kind === "yaml") return parseYaml(fm.value) ?? {};
  if (fm.kind === "toml") return parseToml(fm.value);
  return {};
}

export default function vitePluginSatteri(options: VitePluginSatteriOptions = {}): Plugin {
  const { markdown = true, mdx = true, mdastPlugins, hastPlugins, features } = options;

  const mdxEnabled = mdx !== false;
  const mdxOptions: MdxOptions = typeof mdx === "object" ? mdx : {};

  let viteConfig: ResolvedConfig | undefined;

  return {
    name: "vite-plugin-satteri",
    enforce: "pre",
    configResolved(config) {
      viteConfig = config;
    },
    async transform(source, id) {
      const isMdx = mdxEnabled && MDX_RE.test(id);
      const isMd = !isMdx && markdown && MD_RE.test(id);
      if (!isMd && !isMdx) return null;

      const filePath = id.replace(/\?.*$/, "");
      const fileURL = pathToFileURL(filePath);

      if (isMdx) {
        const isDev = mdxOptions.development ?? viteConfig?.command === "serve";
        const opts: MdxCompileOptions = {
          fileURL,
          development: isDev,
          ...(mdastPlugins ? { mdastPlugins } : {}),
          ...(hastPlugins ? { hastPlugins } : {}),
          ...(features ? { features } : {}),
          ...(mdxOptions.optimizeStatic ? { optimizeStatic: mdxOptions.optimizeStatic } : {}),
          ...(mdxOptions.jsxImportSource !== undefined
            ? { jsxImportSource: mdxOptions.jsxImportSource }
            : {}),
          ...(mdxOptions.jsx !== undefined ? { jsx: mdxOptions.jsx } : {}),
          ...(mdxOptions.jsxRuntime !== undefined ? { jsxRuntime: mdxOptions.jsxRuntime } : {}),
          ...(mdxOptions.providerImportSource !== undefined
            ? { providerImportSource: mdxOptions.providerImportSource }
            : {}),
          ...(mdxOptions.pragma !== undefined ? { pragma: mdxOptions.pragma } : {}),
          ...(mdxOptions.pragmaFrag !== undefined ? { pragmaFrag: mdxOptions.pragmaFrag } : {}),
          ...(mdxOptions.pragmaImportSource !== undefined
            ? { pragmaImportSource: mdxOptions.pragmaImportSource }
            : {}),
        };
        const { code: mdxCode, frontmatter } = await mdxToJs(source, opts);
        const fm = parseFrontmatter(frontmatter);
        const code = `export const frontmatter = ${JSON.stringify(fm)};\n${mdxCode}`;
        return { code, map: null };
      }

      const opts: CompileOptions = {
        fileURL,
        ...(mdastPlugins ? { mdastPlugins } : {}),
        ...(hastPlugins ? { hastPlugins } : {}),
        ...(features ? { features } : {}),
      };
      const { html, frontmatter } = await markdownToHtml(source, opts);
      const fm = parseFrontmatter(frontmatter);
      const code =
        `const html = ${JSON.stringify(html)};\n` +
        `export const frontmatter = ${JSON.stringify(fm)};\n` +
        `export { html };\n` +
        `export default html;\n`;
      return { code, map: null };
    },
  };
}

export { vitePluginSatteri as satteri };
