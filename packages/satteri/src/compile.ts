import { visitHastHandle, resolveSubscriptions, type HastHandle } from "./hast/hast-visitor.js";
import {
  visitMdastHandle,
  resolveMdastSubscriptions,
  type MdastPluginInstance,
} from "./mdast/mdast-visitor.js";
import type { MdastPluginDefinition, HastPluginDefinition } from "./plugin.js";
import {
  parseToHtml,
  compileMdx,
  createHastHandle,
  createMdxHastHandle,
  renderHandle,
  compileHandle,
  applyCommandsToHandle,
  dropHandle,
  createMdastHandle,
  createMdxMdastHandle,
  applyCommandsToMdastHandle,
  convertMdastToHastHandle,
  applyCommandsAndConvertToHastHandle,
  getHandleSource,
  serializeHandle,
  serializeMdastHandle,
} from "#binding";
import { MdastReader } from "./mdast/mdast-reader.js";
import { materializeMdastTree } from "./mdast/mdast-materializer.js";
import { HastReader } from "./hast/hast-reader.js";
import { materializeHastTree } from "./hast/hast-materializer.js";
import type { MdastNode, HastNode } from "./types.js";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type MdastHandle = any;

type MdastPipelineResult = { handle: MdastHandle; pendingCommands: Uint8Array | null };

function runMdastPluginsOnHandle(
  handle: MdastHandle,
  plugins: MdastPluginDefinition[],
  filename: string,
): MdastPipelineResult | Promise<MdastPipelineResult> {
  let pendingCommands: Uint8Array | null = null;
  const source = getHandleSource(handle);

  let i = 0;
  const runNext = (): MdastPipelineResult | Promise<MdastPipelineResult> => {
    while (i < plugins.length) {
      const idx = i++;
      const plugin = plugins[idx]!;
      const subs = resolveMdastSubscriptions(plugin as MdastPluginInstance);
      const result = visitMdastHandle(
        handle,
        plugin as MdastPluginInstance,
        subs,
        source,
        filename,
      );

      if (result instanceof Promise) {
        return result.then((r) => {
          applyMdastResult(r, idx, plugins.length, handle);
          return runNext();
        });
      }

      applyMdastResult(result, idx, plugins.length, handle);
    }
    return { handle, pendingCommands };
  };

  function applyMdastResult(
    result: { commandBuffer: Uint8Array; hasMutations: boolean },
    idx: number,
    total: number,
    h: MdastHandle,
  ) {
    if (result.hasMutations) {
      if (idx === total - 1) {
        pendingCommands = result.commandBuffer;
      } else {
        applyCommandsToMdastHandle(h, result.commandBuffer);
      }
    }
  }

  return runNext();
}

function runHastPluginsOnHandle(
  handle: HastHandle,
  plugins: HastPluginDefinition[],
  source: string,
  filename: string,
): void | Promise<void> {
  if (plugins.length === 0) return;

  let i = 0;
  const runNext = (): void | Promise<void> => {
    while (i < plugins.length) {
      const plugin = plugins[i]!;
      i++;

      const subs = resolveSubscriptions(plugin);
      const result = visitHastHandle(handle, plugin, subs, source, filename);
      if (result instanceof Promise) {
        return result.then(runNext);
      }
    }
  };

  return runNext();
}

// Public API

/** Configuration for static subtree collapsing during MDX compilation. */
export interface OptimizeStaticConfig {
  component: string;
  prop: string;
  wrapPropValue?: boolean;
  ignoreElements?: string[];
}

export interface CompileOptions {
  mdastPlugins?: MdastPluginDefinition[];
  hastPlugins?: HastPluginDefinition[];
  filename?: string;
}

export interface MdxCompileOptions extends CompileOptions {
  optimizeStatic?: OptimizeStaticConfig;
}

export function markdownToHtml(
  source: string,
  options: CompileOptions = {},
): string | Promise<string> {
  const { mdastPlugins = [], hastPlugins = [], filename = "<unknown>" } = options;

  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    return parseToHtml(source);
  }

  const handleResult = createHastHandleFromMdast(source, mdastPlugins, false, filename);

  const finish = (hastHandle: HastHandle): string | Promise<string> => {
    const asyncResult = runHastPluginsOnHandle(hastHandle, hastPlugins, source, filename);
    if (asyncResult instanceof Promise) {
      return asyncResult.then(() => {
        const html = renderHandle(hastHandle);
        dropHandle(hastHandle);
        return html;
      });
    }
    const html = renderHandle(hastHandle);
    dropHandle(hastHandle);
    return html;
  };

  if (handleResult instanceof Promise) {
    return handleResult.then(finish);
  }
  return finish(handleResult);
}

export function mdxToJs(source: string, options: MdxCompileOptions = {}): string | Promise<string> {
  const { mdastPlugins = [], hastPlugins = [], optimizeStatic, filename = "<unknown>" } = options;
  const mdxOptions = optimizeStatic ? { optimizeStatic } : undefined;

  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    return compileMdx(source, mdxOptions);
  }

  const handleResult = createHastHandleFromMdast(source, mdastPlugins, true, filename);

  const finish = (hastHandle: HastHandle): string | Promise<string> => {
    const asyncResult = runHastPluginsOnHandle(hastHandle, hastPlugins, source, filename);
    if (asyncResult instanceof Promise) {
      return asyncResult.then(() => {
        const js = compileHandle(hastHandle, mdxOptions);
        dropHandle(hastHandle);
        return js;
      });
    }
    const js = compileHandle(hastHandle, mdxOptions);
    dropHandle(hastHandle);
    return js;
  };

  if (handleResult instanceof Promise) {
    return handleResult.then(finish);
  }
  return finish(handleResult);
}

// Pipeline: parse → mdast plugins → hast conversion → hast plugins
// All arenas stay in Rust. No intermediate buffer copies to JS.

/** Parse + mdast plugins + convert to HAST handle. */
function createHastHandleFromMdast(
  source: string,
  mdastPlugins: MdastPluginDefinition[],
  mdx: boolean,
  filename: string,
): HastHandle | Promise<HastHandle> {
  if (mdastPlugins.length === 0) {
    return mdx ? createMdxHastHandle(source) : createHastHandle(source);
  }

  const mdastHandle = mdx ? createMdxMdastHandle(source) : createMdastHandle(source);
  const mdastResult = runMdastPluginsOnHandle(mdastHandle, mdastPlugins, filename);

  const convert = (r: MdastPipelineResult): HastHandle => {
    if (r.pendingCommands) {
      return applyCommandsAndConvertToHastHandle(r.handle, r.pendingCommands);
    }
    return convertMdastToHastHandle(r.handle);
  };

  if (mdastResult instanceof Promise) {
    return mdastResult.then(convert);
  }
  return convert(mdastResult);
}

// Step-by-step API: individual pipeline stages with materialized trees

/** Parse Markdown source into a materialized mdast tree. */
export function markdownToMdast(source: string): MdastNode {
  const handle = createMdastHandle(source);
  const buf = serializeMdastHandle(handle);
  return materializeMdastTree(new MdastReader(buf));
}

/** Parse MDX source into a materialized mdast tree. */
export function mdxToMdast(source: string): MdastNode {
  const handle = createMdxMdastHandle(source);
  const buf = serializeMdastHandle(handle);
  return materializeMdastTree(new MdastReader(buf));
}

/** Convert Markdown source to a materialized hast tree. */
export function markdownToHast(source: string): HastNode {
  const handle = createHastHandle(source);
  const buf = serializeHandle(handle);
  dropHandle(handle);
  return materializeHastTree(new HastReader(buf));
}

/** Convert MDX source to a materialized hast tree. */
export function mdxToHast(source: string): HastNode {
  const handle = createMdxHastHandle(source);
  const buf = serializeHandle(handle);
  dropHandle(handle);
  return materializeHastTree(new HastReader(buf));
}
