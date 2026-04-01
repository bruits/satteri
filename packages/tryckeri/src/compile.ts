/**
 * Top-level compile functions — the primary public API.
 *
 * Both MDAST and HAST arenas stay in Rust memory via opaque handles.
 * Only matched nodes and mutation commands cross the NAPI boundary.
 */

import {
  visitHast,
  visitHastHandle,
  resolveSubscriptions,
  type HastHandle,
} from "./hast/hast-visitor.js";
import { HastReader } from "./hast/hast-reader.js";
import { DataMap } from "./data-map.js";
import { MdastReader } from "./mdast/mdast-reader.js";
import {
  visitMdast,
  visitMdastHandle,
  resolveMdastSubscriptions,
  type MdastPluginInstance,
} from "./mdast/mdast-visitor.js";
import { materializeNode } from "./mdast/mdast-materializer.js";
import type { MdastPluginDefinition, HastPluginDefinition } from "./plugin.js";
import type { MdastNode } from "./types.js";
import {
  parseToHtml,
  compileMdx,
  createHastHandle,
  createMdxHastHandle,
  renderHandle,
  compileHandle,
  serializeHandle,
  applyCommandsToHandle,
  dropHandle,
  createMdastHandle,
  createMdxMdastHandle,
  serializeMdastHandle,
  applyCommandsToMdastHandle,
  convertMdastToHastHandle,
  applyCommandsAndConvertToHastHandle,
  getHandleSource,
} from "../index.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function initPlugins<T>(
  plugins: { name: string; createOnce(): T }[],
): { instance: T; name: string }[] {
  return plugins.map((def) => ({
    instance: def.createOnce(),
    name: def.name,
  }));
}

// ---------------------------------------------------------------------------
// MDAST plugin runner (handle-based)
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type MdastHandle = any;

interface FileContext {
  source: string;
  filename: string;
  get root(): MdastNode;
}

function wrapInstance(
  instance: ReturnType<MdastPluginDefinition["createOnce"]>,
  fileContext: FileContext,
): MdastPluginInstance {
  const wrapped: Record<string, unknown> = {};

  for (const [key, val] of Object.entries(instance as Record<string, unknown>)) {
    if (key !== "before" && key !== "after" && key !== "transformRoot") {
      if (typeof val === "function") {
        wrapped[key] = val;
      }
    }
  }

  const inst = instance as Record<string, unknown>;
  if (typeof inst.before === "function") {
    wrapped.before = (visitorContext: unknown) =>
      (inst.before as (fc: FileContext, vc: unknown) => unknown)(fileContext, visitorContext);
  }
  if (typeof inst.after === "function") {
    wrapped.after = (visitorContext: unknown) =>
      (inst.after as (fc: FileContext, vc: unknown) => unknown)(fileContext, visitorContext);
  }
  if (typeof inst.transformRoot === "function") {
    wrapped.transformRoot = (root: MdastNode, visitorContext: unknown) =>
      (inst.transformRoot as (r: MdastNode, fc: FileContext, vc: unknown) => unknown)(
        root,
        fileContext,
        visitorContext,
      );
  }

  return wrapped as MdastPluginInstance;
}

/**
 * Run MDAST plugins on a handle. Returns the (possibly new) handle after
 * the last plugin's mutations, plus any pending commands for the last plugin
 * (for fusion with the HAST conversion step).
 */
function runMdastPluginsOnHandle(
  handle: MdastHandle,
  plugins: MdastPluginDefinition[],
  filename: string,
): { handle: MdastHandle; pendingCommands: Uint8Array | null } {
  const instances = initPlugins(plugins);
  const dm = new DataMap();
  let pendingCommands: Uint8Array | null = null;
  const source = getHandleSource(handle);

  for (let i = 0; i < instances.length; i++) {
    const { instance } = instances[i]!;

    const fileContext: FileContext = {
      source,
      filename,
      get root() {
        // Fallback: materialize from serialized buffer (only for transformRoot)
        const buf = serializeMdastHandle(handle);
        return materializeNode(new MdastReader(buf), 0, dm);
      },
    };

    const wrappedPlugin = wrapInstance(instance, fileContext);
    const subs = resolveMdastSubscriptions(wrappedPlugin);

    let result: { commandBuffer: Uint8Array; hasMutations: boolean };
    if (subs) {
      // Handle path: Rust walks, only matched nodes cross the boundary
      result = visitMdastHandle(handle, wrappedPlugin, subs, dm);
    } else {
      // Buffer fallback: transformRoot plugins need full materialization
      const buf = serializeMdastHandle(handle);
      const reader = new MdastReader(buf);
      result = visitMdast(reader, wrappedPlugin, dm);
    }

    if (result.hasMutations) {
      if (i === instances.length - 1) {
        pendingCommands = result.commandBuffer;
      } else {
        applyCommandsToMdastHandle(handle, result.commandBuffer);
      }
    }
  }

  return { handle, pendingCommands };
}

// ---------------------------------------------------------------------------
// HAST plugin runner (handle-based)
// ---------------------------------------------------------------------------

function runHastPluginsOnHandle(handle: HastHandle, plugins: HastPluginDefinition[]): void {
  if (plugins.length === 0) return;

  const instances = initPlugins(plugins);
  for (const { instance } of instances) {
    const subs = resolveSubscriptions(instance);
    if (subs) {
      visitHastHandle(handle, instance, subs);
    } else {
      // Buffer fallback: transformRoot plugins
      const buf = serializeHandle(handle);
      const result = visitHast(new HastReader(buf), instance, new DataMap());
      if (result.hasMutations) {
        applyCommandsToHandle(handle, result.commandBuffer);
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

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
  optimizeStatic?: OptimizeStaticConfig;
}

export function compileMarkdownToHtml(source: string, options: CompileOptions = {}): string {
  const { mdastPlugins = [], hastPlugins = [] } = options;

  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    return parseToHtml(source);
  }

  const hastHandle = createHastHandleFromPipeline(source, mdastPlugins, hastPlugins, false);
  const html = renderHandle(hastHandle);
  dropHandle(hastHandle);
  return html;
}

export function compileMdxToJs(source: string, options: CompileOptions = {}): string {
  const { mdastPlugins = [], hastPlugins = [], optimizeStatic } = options;
  const mdxOptions = optimizeStatic ? { optimizeStatic } : undefined;

  if (mdastPlugins.length === 0 && hastPlugins.length === 0) {
    return compileMdx(source, mdxOptions);
  }

  const hastHandle = createHastHandleFromPipeline(source, mdastPlugins, hastPlugins, true);
  const js = compileHandle(hastHandle, mdxOptions);
  dropHandle(hastHandle);
  return js;
}

// ---------------------------------------------------------------------------
// Pipeline: parse → mdast plugins → hast conversion → hast plugins
// All arenas stay in Rust. No intermediate buffer copies to JS.
// ---------------------------------------------------------------------------

function createHastHandleFromPipeline(
  source: string,
  mdastPlugins: MdastPluginDefinition[],
  hastPlugins: HastPluginDefinition[],
  mdx: boolean,
): HastHandle {
  let hastHandle: HastHandle;

  if (mdastPlugins.length > 0) {
    // Parse → MDAST handle
    const mdastHandle = mdx ? createMdxMdastHandle(source) : createMdastHandle(source);

    // Run MDAST plugins (arena stays in Rust between plugins)
    const { pendingCommands } = runMdastPluginsOnHandle(mdastHandle, mdastPlugins, "<unknown>");

    // Convert to HAST handle (fuse last plugin's mutations if any)
    if (pendingCommands) {
      hastHandle = applyCommandsAndConvertToHastHandle(mdastHandle, pendingCommands);
    } else {
      hastHandle = convertMdastToHastHandle(mdastHandle);
    }
    // mdastHandle is now empty (consumed by conversion)
  } else {
    hastHandle = mdx ? createMdxHastHandle(source) : createHastHandle(source);
  }

  runHastPluginsOnHandle(hastHandle, hastPlugins);
  return hastHandle;
}
