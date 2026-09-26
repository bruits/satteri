import type { markdownToHtmlFast } from "#binding";
import type { Features, MdxOnlyOptions } from "./compile-types.js";
import {
  ENABLE_DEFINITION_LIST,
  ENABLE_DIRECTIVE,
  ENABLE_FOOTNOTES,
  ENABLE_GFM,
  ENABLE_HEADING_ATTRIBUTES,
  ENABLE_MATH,
  ENABLE_MATH_MULTI_DOLLAR,
  ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS,
  ENABLE_SMART_DASHES,
  ENABLE_SMART_ELLIPSES,
  ENABLE_SMART_PUNCTUATION,
  ENABLE_SMART_QUOTES,
  ENABLE_STRIKETHROUGH,
  ENABLE_SUBSCRIPT,
  ENABLE_SUPERSCRIPT,
  ENABLE_TABLES,
  ENABLE_TASKLISTS,
  ENABLE_WIKILINKS,
  ENABLE_YAML_STYLE_METADATA_BLOCKS,
} from "./generated/parse-options.js";

const GFM_PARSE_OPTIONS = ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_TASKLISTS | ENABLE_GFM;
const FRONTMATTER_PARSE_OPTIONS =
  ENABLE_YAML_STYLE_METADATA_BLOCKS | ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;

export const DEFAULT_PARSE_OPTIONS =
  GFM_PARSE_OPTIONS | ENABLE_FOOTNOTES | FRONTMATTER_PARSE_OPTIONS;

export type NativeConvertOptions = NonNullable<Parameters<typeof markdownToHtmlFast>[2]>;

type NativeCompileOptions = {
  parseOptions: number;
  convertOptions: NativeConvertOptions | undefined;
};

function featuresToParseOptions(features: Features): number {
  let options = 0;

  const { gfm } = features;
  if (typeof gfm === "object") {
    options |= GFM_PARSE_OPTIONS;
    if (gfm.footnotes !== false) options |= ENABLE_FOOTNOTES;
  } else if (gfm ?? true) {
    options |= GFM_PARSE_OPTIONS | ENABLE_FOOTNOTES;
  }

  if (features.frontmatter ?? true) options |= FRONTMATTER_PARSE_OPTIONS;

  const { math } = features;
  if (typeof math === "object") {
    options |= math.singleDollarTextMath === false ? ENABLE_MATH_MULTI_DOLLAR : ENABLE_MATH;
  } else if (math) {
    options |= ENABLE_MATH;
  }

  if (features.headingAttributes) options |= ENABLE_HEADING_ATTRIBUTES;
  if (features.directive) options |= ENABLE_DIRECTIVE;
  if (features.superscript) options |= ENABLE_SUPERSCRIPT;
  if (features.subscript) options |= ENABLE_SUBSCRIPT;
  if (features.wikilinks) options |= ENABLE_WIKILINKS;
  if (features.definitionList) options |= ENABLE_DEFINITION_LIST;

  const smartPunctuation = features.smartPunctuation;
  if (typeof smartPunctuation === "object") {
    if (smartPunctuation.quotes ?? true) options |= ENABLE_SMART_QUOTES;
    if (smartPunctuation.dashes ?? true) options |= ENABLE_SMART_DASHES;
    if (smartPunctuation.ellipses ?? true) options |= ENABLE_SMART_ELLIPSES;
  } else if (smartPunctuation) {
    options |= ENABLE_SMART_PUNCTUATION;
  }

  return options;
}

export function featuresToNative(features: Features | undefined): NativeCompileOptions {
  if (!features) return { parseOptions: DEFAULT_PARSE_OPTIONS, convertOptions: undefined };
  let convertOptions: NativeConvertOptions | undefined;

  const { gfm } = features;
  if (typeof gfm === "object" && typeof gfm.footnotes === "object") {
    const { label, backContent, backLabel, clobberPrefix } = gfm.footnotes;
    convertOptions = {};
    if (label !== undefined) convertOptions.footnoteLabel = label;
    if (backContent !== undefined) convertOptions.footnoteBackContent = backContent;
    if (backLabel !== undefined) convertOptions.footnoteBackLabel = backLabel;
    if (clobberPrefix !== undefined) convertOptions.clobberPrefix = clobberPrefix;
  }
  if (features.rawHtml !== undefined) {
    // Conversion options must carry rawHtml so tree APIs and compile pipelines apply the same reparse.
    convertOptions = convertOptions ?? {};
    convertOptions.rawHtml = features.rawHtml;
  }
  return { parseOptions: featuresToParseOptions(features), convertOptions };
}

const MDX_NATIVE_OPTION_KEYS = [
  "optimizeStatic",
  "jsxImportSource",
  "jsx",
  "jsxRuntime",
  "development",
  "providerImportSource",
  "pragma",
  "pragmaFrag",
  "pragmaImportSource",
  "outputFormat",
  "elementAttributeNameCase",
  "stylePropertyNameCase",
] as const;

export function mdxOptionsToNative(opts: MdxOnlyOptions): MdxOnlyOptions | undefined {
  let result: MdxOnlyOptions | undefined;
  const copyOption = <K extends keyof MdxOnlyOptions>(key: K): void => {
    const value = opts[key];
    if (value === undefined || (key === "optimizeStatic" && !value)) return;
    (result ??= {})[key] = value;
  };
  MDX_NATIVE_OPTION_KEYS.forEach(copyOption);
  return result;
}
