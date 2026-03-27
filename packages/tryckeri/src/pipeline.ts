import { visitMdast } from "./visitor.js";
import { DataMap } from "./data-map.js";
import { MdastReader } from "./mdast-reader.js";
import { materializeNode } from "./materializer.js";
import type { MdastPluginDefinition } from "./plugin.js";
import type { MdastNode } from "./types.js";
import type { Diagnostic } from "./visitor.js";

// applyMutations is the NAPI function that takes an arena buffer + command
// buffer and returns a new arena buffer with all mutations applied.
import { applyMutations } from "../index.js";

export class ProcessorContext {
  readonly #diagnostics: Diagnostic[] = [];

  report(diagnostic: Diagnostic): void {
    this.#diagnostics.push(diagnostic);
  }

  getDiagnostics(): Diagnostic[] {
    return [...this.#diagnostics];
  }
}

export interface FileContext {
  source: string;
  filename: string;
  get root(): MdastNode;
}

export interface RunResult {
  buffer: ArrayBuffer | Uint8Array;
  dataMap: DataMap;
  diagnostics: Diagnostic[];
  mutationCount: number;
  structuralMutationCount: number;
}

/**
 * Process one arena buffer through an ordered list of initialized plugin instances.
 */
export async function runPluginsOnBuffer(
  buffer: ArrayBuffer | Uint8Array,
  pluginInstances: { instance: ReturnType<MdastPluginDefinition["createOnce"]>; name: string }[],
  { filename = "<unknown>", dataMap }: { filename?: string; dataMap?: DataMap } = {},
): Promise<RunResult> {
  const dm = dataMap ?? new DataMap();
  const allDiagnostics: Diagnostic[] = [];
  let totalMutations = 0;
  let structuralMutations = 0;
  let currentBuffer = buffer;

  for (const { instance, name: _name } of pluginInstances) {
    const reader = new MdastReader(currentBuffer);

    const fileContext: FileContext = {
      source: reader.getSource(),
      filename,
      get root() {
        return materializeNode(reader, 0, dm);
      },
    };

    const wrappedPlugin = wrapInstance(instance, fileContext);
    const result = await visitMdast(reader, wrappedPlugin, dm);
    allDiagnostics.push(...result.diagnostics);

    if (result.hasMutations) {
      totalMutations += 1;
      structuralMutations += 1;

      // Send the binary command buffer to Rust for application
      const arenaBuf =
        currentBuffer instanceof Uint8Array
          ? currentBuffer
          : new Uint8Array(currentBuffer);
      const newBuffer = applyMutations(arenaBuf, result.commandBuffer);
      currentBuffer = newBuffer;
    }
  }

  return {
    buffer: currentBuffer,
    dataMap: dm,
    diagnostics: allDiagnostics,
    mutationCount: totalMutations,
    structuralMutationCount: structuralMutations,
  };
}

function wrapInstance(
  instance: ReturnType<MdastPluginDefinition["createOnce"]>,
  fileContext: FileContext,
): ReturnType<MdastPluginDefinition["createOnce"]> {
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

  return wrapped as ReturnType<MdastPluginDefinition["createOnce"]>;
}
