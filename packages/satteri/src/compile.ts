import { featuresToNative, mdxOptionsToNative } from "./compile-options.js";
import type {
  CompileOptions,
  EvaluateOptions,
  Frontmatter,
  HastTreeOptions,
  HtmlToHastOptions,
  MarkdownToHtmlResult,
  MarkdownToJsOptions,
  MarkdownToJsResult,
  MdastTreeOptions,
  MdxCompileOptions,
  MdxToJsResult,
  ResultFor,
} from "./compile-types.js";
import type { HastHandle, MdastHandle } from "./handles.js";
import {
  createHastHandleFromMdast,
  EMPTY_COMMAND_BUFFER,
  releaseHandle,
  runHastPluginsCollectLast,
  runMdastPluginsOnHandle,
  warnIfDroppedTransforms,
  type CollectedHastCommands,
  type HastWithFrontmatter,
  type MdastPipelineResult,
} from "./plugin-pipeline.js";
export { DEFAULT_PARSE_OPTIONS, featuresToNative } from "./compile-options.js";
export type {
  CompileOptions,
  EvaluateOptions,
  Features,
  FootnoteBackrefCallback,
  FootnoteOptions,
  Frontmatter,
  GfmOptions,
  HastTreeOptions,
  HtmlToHastOptions,
  MarkdownToHtmlResult,
  MarkdownToJsOptions,
  MarkdownToJsResult,
  MathOptions,
  MdastTreeOptions,
  MdxCompileOptions,
  MdxOnlyOptions,
  MdxToJsResult,
  OptimizeStaticConfig,
  SmartPunctuationOptions,
  TreeOptions,
} from "./compile-types.js";

import {
  applyCommandsAndCompileHandle,
  applyCommandsAndRenderHandle,
  applyCommandsToHandle,
  applyMdastCommandsAndConvertAndCompile,
  applyMdastCommandsAndConvertAndRender,
  compileHandle,
  createHastHandleFromHtml,
  createMdastHandle,
  createMdxMdastHandle,
  markdownToHtmlFast,
  markdownToJsFast,
  mdxToJsFast,
  parseHastWire,
  parseMdastWire,
  renderHandle,
  serializeHandle,
} from "#binding";
import { materializeHastTree } from "./hast/hast-materializer.js";
import { HastReader } from "./hast/hast-reader.js";
import { materializeMdastTree } from "./mdast/mdast-materializer.js";
import { MdastReader } from "./mdast/mdast-reader.js";
import { normalizePlugins } from "./plugin.js";
import type { HastNode, MdastNode, SourceFormat } from "./types.js";

export function markdownToHtml(source: string): MarkdownToHtmlResult;
export function markdownToHtml<O extends CompileOptions>(
  source: string,
  options?: O,
): ResultFor<O, MarkdownToHtmlResult>;
export function markdownToHtml(
  source: string,
  options: CompileOptions = {},
): MarkdownToHtmlResult | Promise<MarkdownToHtmlResult> {
  const { features, fileURL, data = {} } = options;
  const mdastPlugins = normalizePlugins(
    options.mdastPlugins ?? [],
    "mdastPlugins",
    source,
    fileURL,
    "markdown",
    data,
  );
  const hastPlugins = normalizePlugins(
    options.hastPlugins ?? [],
    "hastPlugins",
    source,
    fileURL,
    "markdown",
    data,
  );
  const hastMayHaveStubs = hastPlugins.length > 0;
  const { parseOptions, convertOptions: nativeConvertOptions } = featuresToNative(features);

  // Without plugins, one NAPI call avoids the handle lifecycle’s extra boundary crossings.
  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    const { html, frontmatter } = markdownToHtmlFast(source, parseOptions, nativeConvertOptions);
    return { html, frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null, data };
  }

  // Skip position tracking unless requested to avoid line-index construction and per-node lookups.
  const trackPositions =
    mdastPlugins.some((p) => p.options?.position) || hastPlugins.some((p) => p.options?.position);

  // Without HAST plugins, fuse apply, conversion, and rendering to avoid extra NAPI crossings.
  if (hastPlugins.length === 0) {
    const mdastHandle = createMdastHandle(source, parseOptions, trackPositions);
    try {
      const mdastResult = runMdastPluginsOnHandle(
        mdastHandle,
        mdastPlugins,
        fileURL,
        data,
        "markdown",
        true,
      );
      const finishMdast = (r: MdastPipelineResult): MarkdownToHtmlResult => {
        try {
          const commands = r.pendingCommands ?? EMPTY_COMMAND_BUFFER;
          const { html, frontmatter, droppedTransforms } = applyMdastCommandsAndConvertAndRender(
            r.handle,
            commands,
            nativeConvertOptions,
          );
          warnIfDroppedTransforms(droppedTransforms, r.lastPlugin, "mdast");
          return {
            html,
            frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null,
            data,
          };
        } finally {
          releaseHandle(r.handle, true);
        }
      };
      if (mdastResult instanceof Promise) {
        return mdastResult.then(finishMdast, (err) => {
          releaseHandle(mdastHandle, true);
          throw err;
        });
      }
      return finishMdast(mdastResult);
    } catch (err) {
      releaseHandle(mdastHandle, true);
      throw err;
    }
  }

  const result = createHastHandleFromMdast(
    source,
    mdastPlugins,
    false,
    fileURL,
    parseOptions,
    nativeConvertOptions,
    data,
    trackPositions,
  );

  const runHastThenRender = (
    r: HastWithFrontmatter,
  ): MarkdownToHtmlResult | Promise<MarkdownToHtmlResult> => {
    let collected: CollectedHastCommands | Promise<CollectedHastCommands>;
    try {
      collected = runHastPluginsCollectLast(
        r.hastHandle,
        hastPlugins,
        source,
        fileURL,
        data,
        "markdown",
      );
    } catch (err) {
      releaseHandle(r.hastHandle, hastMayHaveStubs);
      throw err;
    }
    if (collected instanceof Promise) {
      return collected.then(
        (c) => finishHastRender(r.hastHandle, c, r.frontmatter),
        (err) => {
          releaseHandle(r.hastHandle, hastMayHaveStubs);
          throw err;
        },
      );
    }
    return finishHastRender(r.hastHandle, collected, r.frontmatter);
  };

  const finishHastRender = (
    h: HastHandle,
    collected: CollectedHastCommands,
    frontmatter: Frontmatter | null,
  ): MarkdownToHtmlResult => {
    try {
      let html: string;
      if (collected.commands.length > 0) {
        const transformed = applyCommandsAndRenderHandle(h, collected.commands);
        warnIfDroppedTransforms(transformed.droppedTransforms, collected.lastPlugin, "hast");
        html = transformed.html;
      } else {
        html = renderHandle(h);
      }
      return { html, frontmatter, data };
    } finally {
      releaseHandle(h, hastMayHaveStubs);
    }
  };

  if (result instanceof Promise) return result.then(runHastThenRender);
  return runHastThenRender(result);
}

export function mdxToJs(source: string): MdxToJsResult;
export function mdxToJs<O extends MdxCompileOptions>(
  source: string,
  options?: O,
): ResultFor<O, MdxToJsResult>;
export function mdxToJs(
  source: string,
  options: MdxCompileOptions = {},
): MdxToJsResult | Promise<MdxToJsResult> {
  return toJsImpl(source, options, true);
}

/**
 * Compile plain Markdown to a JavaScript module: like {@link mdxToJs}, but
 * without MDX syntax: `{...}` expressions, JSX tags, and `import`/`export`
 * lines are ordinary Markdown. HTML has no JSX representation and is dropped;
 * enable `features: { rawHtml: true }` to parse it into real elements instead.
 */
export function markdownToJs(source: string): MarkdownToJsResult;
export function markdownToJs<O extends MarkdownToJsOptions>(
  source: string,
  options?: O,
): ResultFor<O, MarkdownToJsResult>;
export function markdownToJs(
  source: string,
  options: MarkdownToJsOptions = {},
): MarkdownToJsResult | Promise<MarkdownToJsResult> {
  return toJsImpl(source, options, false);
}

function toJsImpl(
  source: string,
  options: MdxCompileOptions,
  mdx: boolean,
): MdxToJsResult | Promise<MdxToJsResult> {
  const {
    mdastPlugins: mdastInput = [],
    hastPlugins: hastInput = [],
    features,
    fileURL,
    data = {},
    ...mdxFields
  } = options;
  const sourceFormat: SourceFormat = mdx ? "mdx" : "markdown";
  const mdastPlugins = normalizePlugins(
    mdastInput,
    "mdastPlugins",
    source,
    fileURL,
    sourceFormat,
    data,
  );
  const hastPlugins = normalizePlugins(
    hastInput,
    "hastPlugins",
    source,
    fileURL,
    sourceFormat,
    data,
  );
  const hastMayHaveStubs = hastPlugins.length > 0;
  const mdxOptions = mdxOptionsToNative(mdxFields);
  const { parseOptions, convertOptions: nativeConvertOptions } = featuresToNative(features);

  // Without plugins, one NAPI call avoids the handle lifecycle’s extra boundary crossings.
  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    const { code, frontmatter } = (mdx ? mdxToJsFast : markdownToJsFast)(
      source,
      parseOptions,
      mdxOptions,
      nativeConvertOptions,
    );
    return { code, frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null, data };
  }

  const trackPositions =
    mdastPlugins.some((p) => p.options?.position) || hastPlugins.some((p) => p.options?.position);

  // Without HAST plugins, fuse apply, conversion, and compilation to avoid extra NAPI crossings.
  if (hastPlugins.length === 0) {
    const mdastHandle = mdx
      ? createMdxMdastHandle(source, parseOptions, trackPositions)
      : createMdastHandle(source, parseOptions, trackPositions);
    try {
      const mdastResult = runMdastPluginsOnHandle(
        mdastHandle,
        mdastPlugins,
        fileURL,
        data,
        mdx ? "mdx" : "markdown",
        true,
      );
      const finishMdast = (r: MdastPipelineResult): MdxToJsResult => {
        try {
          const commands = r.pendingCommands ?? EMPTY_COMMAND_BUFFER;
          const { code, frontmatter, droppedTransforms } = applyMdastCommandsAndConvertAndCompile(
            r.handle,
            commands,
            mdxOptions,
            nativeConvertOptions,
          );
          warnIfDroppedTransforms(droppedTransforms, r.lastPlugin, "mdast");
          return {
            code,
            frontmatter: (frontmatter as Frontmatter | null | undefined) ?? null,
            data,
          };
        } finally {
          releaseHandle(r.handle, true);
        }
      };
      if (mdastResult instanceof Promise) {
        return mdastResult.then(finishMdast, (err) => {
          releaseHandle(mdastHandle, true);
          throw err;
        });
      }
      return finishMdast(mdastResult);
    } catch (err) {
      releaseHandle(mdastHandle, true);
      throw err;
    }
  }

  const result = createHastHandleFromMdast(
    source,
    mdastPlugins,
    mdx,
    fileURL,
    parseOptions,
    nativeConvertOptions,
    data,
    trackPositions,
  );

  const runHastThenCompile = (r: HastWithFrontmatter): MdxToJsResult | Promise<MdxToJsResult> => {
    let collected: CollectedHastCommands | Promise<CollectedHastCommands>;
    try {
      collected = runHastPluginsCollectLast(
        r.hastHandle,
        hastPlugins,
        source,
        fileURL,
        data,
        mdx ? "mdx" : "markdown",
      );
    } catch (err) {
      releaseHandle(r.hastHandle, hastMayHaveStubs);
      throw err;
    }
    if (collected instanceof Promise) {
      return collected.then(
        (c) => finishHastCompile(r.hastHandle, c, r.frontmatter),
        (err) => {
          releaseHandle(r.hastHandle, hastMayHaveStubs);
          throw err;
        },
      );
    }
    return finishHastCompile(r.hastHandle, collected, r.frontmatter);
  };

  const finishHastCompile = (
    h: HastHandle,
    collected: CollectedHastCommands,
    frontmatter: Frontmatter | null,
  ): MdxToJsResult => {
    try {
      let code: string;
      if (collected.commands.length > 0) {
        const transformed = applyCommandsAndCompileHandle(h, collected.commands, mdxOptions);
        warnIfDroppedTransforms(transformed.droppedTransforms, collected.lastPlugin, "hast");
        code = transformed.code;
      } else {
        code = compileHandle(h, mdxOptions);
      }
      return { code, frontmatter, data };
    } finally {
      releaseHandle(h, hastMayHaveStubs);
    }
  };

  if (result instanceof Promise) return result.then(runHastThenCompile);
  return runHastThenCompile(result);
}

/**
 * Compile and evaluate MDX in one step.
 *
 * Returns the module's exports, including `default` (the MDX component).
 * Returns a Promise when async plugins are used, otherwise returns synchronously.
 *
 * ```ts
 * import * as runtime from "react/jsx-runtime";
 * const { default: Content } = evaluate("# Hello", { ...runtime });
 * ```
 */
export function evaluate(
  source: string,
  options: EvaluateOptions,
): Record<string, unknown> | Promise<Record<string, unknown>> {
  const { Fragment, jsx, jsxs, jsxDEV, useMDXComponents, ...compileOpts } = options;
  const runtime = { Fragment, jsx, jsxs, jsxDEV, useMDXComponents };
  const result = mdxToJs(source, { ...compileOpts, outputFormat: "function-body" });
  if (result instanceof Promise) {
    return result.then((resolved) => new Function(resolved.code)(runtime));
  }
  return new Function(result.code)(runtime);
}

function materializeMdastHandle(handle: MdastHandle): MdastNode {
  try {
    return materializeMdastTree(new MdastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}

function materializeHastHandle(handle: HastHandle, invalidateStubs: boolean): HastNode {
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, invalidateStubs);
  }
}

function applyCollectedHastCommands(
  handle: HastHandle,
  collected: CollectedHastCommands,
  invalidateStubs: boolean,
): HastNode {
  if (collected.commands.length > 0) {
    try {
      const dropped = applyCommandsToHandle(handle, collected.commands);
      warnIfDroppedTransforms(dropped, collected.lastPlugin, "hast");
    } catch (err) {
      releaseHandle(handle, invalidateStubs);
      throw err;
    }
  }
  return materializeHastHandle(handle, invalidateStubs);
}

function toMdastImpl(
  source: string,
  options: MdastTreeOptions,
  mdx: boolean,
): MdastNode | Promise<MdastNode> {
  const { features, fileURL, data = {} } = options;
  const sourceFormat: SourceFormat = mdx ? "mdx" : "markdown";
  const mdastPlugins = normalizePlugins(
    options.mdastPlugins ?? [],
    "mdastPlugins",
    source,
    fileURL,
    sourceFormat,
    data,
  );
  const { parseOptions } = featuresToNative(features);

  if (mdastPlugins.length === 0) {
    const wire = parseMdastWire(source, parseOptions, mdx, options.position);
    return materializeMdastTree(new MdastReader(wire));
  }

  // Tree callers control positions independently of plugin requirements.
  const trackPositions = options.position ?? true;
  const handle = mdx
    ? createMdxMdastHandle(source, parseOptions, trackPositions)
    : createMdastHandle(source, parseOptions, trackPositions);

  try {
    const result = runMdastPluginsOnHandle(handle, mdastPlugins, fileURL, data, sourceFormat);
    if (result instanceof Promise) {
      return result.then(
        (r) => materializeMdastHandle(r.handle),
        (err) => {
          releaseHandle(handle, true);
          throw err;
        },
      );
    }
    return materializeMdastHandle(result.handle);
  } catch (err) {
    releaseHandle(handle, true);
    throw err;
  }
}

function toHastImpl(
  source: string,
  options: HastTreeOptions,
  mdx: boolean,
): HastNode | Promise<HastNode> {
  const { features, fileURL, data = {} } = options;
  const sourceFormat: SourceFormat = mdx ? "mdx" : "markdown";
  const mdastPlugins = normalizePlugins(
    options.mdastPlugins ?? [],
    "mdastPlugins",
    source,
    fileURL,
    sourceFormat,
    data,
  );
  const hastPlugins = normalizePlugins(
    options.hastPlugins ?? [],
    "hastPlugins",
    source,
    fileURL,
    sourceFormat,
    data,
  );
  const { parseOptions, convertOptions } = featuresToNative(features);

  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    const wire = parseHastWire(source, parseOptions, convertOptions, mdx, options.position);
    return materializeHastTree(new HastReader(wire));
  }

  const trackPositions = options.position ?? true;
  const hastMayHaveStubs = hastPlugins.length > 0;

  const runHastPluginsAndMaterialize = (handle: HastHandle): HastNode | Promise<HastNode> => {
    if (hastPlugins.length === 0) return materializeHastHandle(handle, hastMayHaveStubs);

    let collected: CollectedHastCommands | Promise<CollectedHastCommands>;
    try {
      collected = runHastPluginsCollectLast(
        handle,
        hastPlugins,
        source,
        fileURL,
        data,
        sourceFormat,
      );
    } catch (err) {
      releaseHandle(handle, hastMayHaveStubs);
      throw err;
    }
    if (collected instanceof Promise) {
      return collected.then(
        (c) => applyCollectedHastCommands(handle, c, hastMayHaveStubs),
        (err) => {
          releaseHandle(handle, hastMayHaveStubs);
          throw err;
        },
      );
    }
    return applyCollectedHastCommands(handle, collected, hastMayHaveStubs);
  };

  const result = createHastHandleFromMdast(
    source,
    mdastPlugins,
    mdx,
    fileURL,
    parseOptions,
    convertOptions,
    data,
    trackPositions,
  );
  if (result instanceof Promise)
    return result.then((r) => runHastPluginsAndMaterialize(r.hastHandle));
  return runHastPluginsAndMaterialize(result.hastHandle);
}

/** Parse Markdown source into a materialized mdast tree. */
export function markdownToMdast(source: string): MdastNode;
export function markdownToMdast<O extends MdastTreeOptions>(
  source: string,
  options?: O,
): ResultFor<O, MdastNode>;
export function markdownToMdast(
  source: string,
  options: MdastTreeOptions = {},
): MdastNode | Promise<MdastNode> {
  return toMdastImpl(source, options, false);
}

/** Parse MDX source into a materialized mdast tree. */
export function mdxToMdast(source: string): MdastNode;
export function mdxToMdast<O extends MdastTreeOptions>(
  source: string,
  options?: O,
): ResultFor<O, MdastNode>;
export function mdxToMdast(
  source: string,
  options: MdastTreeOptions = {},
): MdastNode | Promise<MdastNode> {
  return toMdastImpl(source, options, true);
}

/** Convert Markdown source to a materialized hast tree. */
export function markdownToHast(source: string): HastNode;
export function markdownToHast<O extends HastTreeOptions>(
  source: string,
  options?: O,
): ResultFor<O, HastNode>;
export function markdownToHast(
  source: string,
  options: HastTreeOptions = {},
): HastNode | Promise<HastNode> {
  return toHastImpl(source, options, false);
}

/** Convert MDX source to a materialized hast tree. */
export function mdxToHast(source: string): HastNode;
export function mdxToHast<O extends HastTreeOptions>(
  source: string,
  options?: O,
): ResultFor<O, HastNode>;
export function mdxToHast(
  source: string,
  options: HastTreeOptions = {},
): HastNode | Promise<HastNode> {
  return toHastImpl(source, options, true);
}

/**
 * Parse an HTML string into a materialized hast tree: a `root` whose children
 * are the doctype (if any) and the implied `<html>` subtree, or the string's
 * own top-level nodes with `{ fragment: true }`. Only available in builds that
 * include the `from-html` feature.
 */
export function htmlToHast(html: string, options: HtmlToHastOptions = {}): HastNode {
  const handle = createHastHandleFromHtml(html, options.fragment, options.space);
  try {
    return materializeHastTree(new HastReader(serializeHandle(handle)));
  } finally {
    releaseHandle(handle, true);
  }
}
