import {
  applyCommandsToHandle,
  applyCommandsToMdastHandle,
  convertMdastToHastHandle,
  createHastHandleWithFrontmatter,
  createMdastHandle,
  createMdxHastHandleWithFrontmatter,
  createMdxMdastHandle,
  dropHandle,
  getHandleSource,
  getMdastFrontmatter,
} from "#binding";
import type { NativeConvertOptions } from "./compile-options.js";
import type { Frontmatter } from "./compile-types.js";
import type { AnyHandle } from "./handles.js";
import {
  resolveSubscriptions,
  visitHastHandleCollect,
  visitHastHookCollect,
  type HastDiagnostic,
  type HastHandle,
  type HastHookFn,
} from "./hast/hast-visitor.js";
import { markHandleMutated } from "./lazy-child-resolver.js";
import {
  resolveMdastSubscriptions,
  visitMdastHandle,
  visitMdastHook,
  type MdastDiagnostic,
  type MdastHandle,
  type MdastHookFn,
} from "./mdast/mdast-visitor.js";
import type { HastPluginDefinition, MdastPluginDefinition } from "./plugin.js";
import type { Data, SourceFormat } from "./types.js";

export type MdastPipelineResult = {
  handle: MdastHandle;
  // Deferring the final apply lets the caller fuse it with rendering or compilation.
  pendingCommands?: Uint8Array;
  // Attribute dropped-transform warnings to the plugin whose apply was deferred.
  lastPlugin?: { name?: string };
};

// Invalidate retained child stubs before their arena is freed.
export function releaseHandle(handle: AnyHandle, invalidateStubs: boolean): void {
  if (invalidateStubs) markHandleMutated(handle);
  dropHandle(handle);
}

export function warnIfDroppedTransforms(
  dropped: number | undefined,
  plugin: { name?: string } | null | undefined,
  kind: "mdast" | "hast",
): void {
  if (!dropped || !plugin) return;
  const name = plugin.name ?? "<anonymous>";
  const noun = dropped === 1 ? "transform" : "transforms";
  console.warn(
    `satteri: plugin "${name}" queued ${dropped} ${kind} ${noun} on node(s) that a plugin had ` +
      `already removed or replaced; ${dropped === 1 ? "it was" : "they were"} dropped.`,
  );
}

function settle<T>(value: T | Promise<T>, fn: (value: T) => void): Promise<void> | undefined {
  if (value instanceof Promise) return value.then(fn);
  fn(value);
  return undefined;
}

// Keep synchronous pipelines synchronous even when later steps can return promises.
function sequence(
  steps: ((() => Promise<void> | undefined) | undefined)[],
): Promise<void> | undefined {
  let pending: Promise<void> | undefined;
  for (const step of steps) {
    if (step === undefined) continue;
    pending = pending === undefined ? step() : pending.then(step);
  }
  return pending;
}

export function runMdastPluginsOnHandle(
  handle: MdastHandle,
  plugins: MdastPluginDefinition[],
  fileURL: URL | undefined,
  data: Data,
  sourceFormat: SourceFormat,
  collectLast = false,
): MdastPipelineResult | Promise<MdastPipelineResult> {
  const out: MdastPipelineResult = { handle };
  const getSource = (): string => getHandleSource(handle);

  const runPlugin = (
    plugin: MdastPluginDefinition,
    isLastPlugin: boolean,
  ): void | Promise<void> => {
    const subs = resolveMdastSubscriptions(plugin);
    const apply = (
      r: { commandBuffer: Uint8Array; hasMutations: boolean },
      isFinalPass: boolean,
    ): void => {
      if (!r.hasMutations) return;
      if (collectLast && isLastPlugin && isFinalPass) {
        out.pendingCommands = r.commandBuffer;
        out.lastPlugin = plugin;
        return;
      }
      markHandleMutated(handle);
      const dropped = applyCommandsToMdastHandle(handle, r.commandBuffer);
      warnIfDroppedTransforms(dropped, plugin, "mdast");
    };

    const before = typeof plugin.before === "function" ? plugin.before : undefined;
    const after = typeof plugin.after === "function" ? plugin.after : undefined;
    const hasVisitors = subs.length > 0;
    // Resolvers are epoch-bound, so each pass needs its own context.
    const diagnostics: MdastDiagnostic[] = [];
    const runHook = (hook: MdastHookFn, isFinalPass: boolean) => () =>
      settle(
        visitMdastHook(handle, plugin, hook, getSource, fileURL, data, sourceFormat, diagnostics),
        (r) => apply(r, isFinalPass),
      );
    const runVisitors = (isFinalPass: boolean) => () =>
      settle(
        visitMdastHandle(handle, plugin, subs, getSource, fileURL, data, sourceFormat, diagnostics),
        (r) => apply(r, isFinalPass),
      );

    return sequence([
      before ? runHook(before, !hasVisitors && after === undefined) : undefined,
      hasVisitors ? runVisitors(after === undefined) : undefined,
      after ? runHook(after, true) : undefined,
    ]);
  };

  let i = 0;
  const runNext = (): MdastPipelineResult | Promise<MdastPipelineResult> => {
    for (;;) {
      const plugin = plugins[i];
      if (plugin === undefined) break;
      i++;
      const r = runPlugin(plugin, i === plugins.length);
      if (r instanceof Promise) return r.then(runNext);
    }
    return out;
  };

  return runNext();
}

export const EMPTY_COMMAND_BUFFER = new Uint8Array(0);

export type CollectedHastCommands = {
  commands: Uint8Array;
  // Attribute dropped-transform warnings to the plugin whose apply was deferred.
  lastPlugin: { name?: string } | null;
};

const NO_HAST_COMMANDS: CollectedHastCommands = {
  commands: EMPTY_COMMAND_BUFFER,
  lastPlugin: null,
};

// Fuse the last apply with rendering or compilation to save a NAPI roundtrip.
export function runHastPluginsCollectLast(
  handle: HastHandle,
  plugins: HastPluginDefinition[],
  source: string,
  fileURL: URL | undefined,
  data: Data,
  sourceFormat: SourceFormat,
): CollectedHastCommands | Promise<CollectedHastCommands> {
  let i = 0;
  const runNext = (): CollectedHastCommands | Promise<CollectedHastCommands> => {
    for (;;) {
      const plugin = plugins[i];
      if (plugin === undefined) break;
      const isLastPlugin = i === plugins.length - 1;
      i++;
      const subs = resolveSubscriptions(plugin);
      const { before, after } = plugin;

      const passes: (HastHookFn | "visitors")[] = [];
      if (typeof before === "function") passes.push(before);
      if (subs.length > 0) passes.push("visitors");
      if (typeof after === "function") passes.push(after);
      if (passes.length === 0) continue;

      let collected = NO_HAST_COMMANDS;
      // Resolvers are epoch-bound, so each pass needs its own context.
      const diagnostics: HastDiagnostic[] = [];
      const runPass = (pass: HastHookFn | "visitors", passIndex: number) => () =>
        settle(
          pass === "visitors"
            ? visitHastHandleCollect(
                handle,
                plugin,
                subs,
                source,
                fileURL,
                data,
                sourceFormat,
                diagnostics,
              )
            : visitHastHookCollect(
                handle,
                plugin,
                pass,
                source,
                fileURL,
                data,
                sourceFormat,
                diagnostics,
              ),
          (commands) => {
            if (isLastPlugin && passIndex === passes.length - 1) {
              collected = { commands, lastPlugin: plugin };
            } else if (commands.length > 0) {
              markHandleMutated(handle);
              const dropped = applyCommandsToHandle(handle, commands);
              warnIfDroppedTransforms(dropped, plugin, "hast");
            }
          },
        );
      const pending = sequence(passes.map(runPass));

      if (isLastPlugin)
        return pending instanceof Promise ? pending.then(() => collected) : collected;
      if (pending instanceof Promise) return pending.then(runNext);
    }
    return NO_HAST_COMMANDS;
  };

  return runNext();
}

export type HastWithFrontmatter = { hastHandle: HastHandle; frontmatter: Frontmatter | null };

function readFrontmatter(handle: MdastHandle): Frontmatter | null {
  const raw = getMdastFrontmatter(handle);
  return raw ? { kind: raw.kind === "toml" ? "toml" : "yaml", value: raw.value } : null;
}

// Read frontmatter after plugins so their YAML/TOML edits reach the result.
export function createHastHandleFromMdast(
  source: string,
  mdastPlugins: MdastPluginDefinition[],
  mdx: boolean,
  fileURL: URL | undefined,
  parseOptions: number,
  nativeConvertOptions: NativeConvertOptions | undefined,
  data: Data,
  trackPositions: boolean,
): HastWithFrontmatter | Promise<HastWithFrontmatter> {
  if (mdastPlugins.length === 0) {
    const [hastHandle, raw] = mdx
      ? createMdxHastHandleWithFrontmatter(
          source,
          parseOptions,
          nativeConvertOptions,
          trackPositions,
        )
      : createHastHandleWithFrontmatter(source, parseOptions, nativeConvertOptions, trackPositions);
    return {
      hastHandle,
      frontmatter: raw ? { kind: raw.kind === "toml" ? "toml" : "yaml", value: raw.value } : null,
    };
  }

  const mdastHandle = mdx
    ? createMdxMdastHandle(source, parseOptions, trackPositions)
    : createMdastHandle(source, parseOptions, trackPositions);
  const sourceFormat: SourceFormat = mdx ? "mdx" : "markdown";

  const mdastMayHaveStubs = mdastPlugins.length > 0;

  // Conversion empties the arena on success; finally also releases it on failure.
  const finalize = (r: MdastPipelineResult): HastWithFrontmatter => {
    try {
      const frontmatter = readFrontmatter(r.handle);
      // Conversion empties the MDAST arena, invalidating retained child stubs.
      if (mdastMayHaveStubs) markHandleMutated(r.handle);
      const hastHandle = convertMdastToHastHandle(r.handle, nativeConvertOptions);
      return { hastHandle, frontmatter };
    } finally {
      releaseHandle(r.handle, mdastMayHaveStubs);
    }
  };

  try {
    const mdastResult = runMdastPluginsOnHandle(
      mdastHandle,
      mdastPlugins,
      fileURL,
      data,
      sourceFormat,
    );

    if (mdastResult instanceof Promise) {
      return mdastResult.then(finalize, (err) => {
        releaseHandle(mdastHandle, mdastMayHaveStubs);
        throw err;
      });
    }
    return finalize(mdastResult);
  } catch (err) {
    releaseHandle(mdastHandle, mdastMayHaveStubs);
    throw err;
  }
}
