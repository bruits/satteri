import { ProcessorContext, runPluginsOnBuffer } from "./pipeline.js";
import { MdastReader } from "./mdast-reader.js";
import { materializeTree } from "./materializer.js";
import { DataMap } from "./data-map.js";
import type { MdastPluginDefinition } from "./plugin.js";
import type { MdastNode } from "./types.js";
import type { Diagnostic } from "./visitor.js";

export { ProcessorContext };

export interface ProcessBufferResult {
  buffer: ArrayBuffer | Uint8Array;
  dataMap: DataMap;
  diagnostics: Diagnostic[];
  mutationCount: number;
  structuralMutationCount: number;
}

export interface ProcessTreeResult {
  tree: MdastNode;
  dataMap: DataMap;
  diagnostics: Diagnostic[];
  mutationCount: number;
}

export function createProcessor({
  plugins = [],
}: { plugins?: MdastPluginDefinition[] } = {}): Processor {
  return new Processor(plugins);
}

class Processor {
  readonly #pluginDefs: MdastPluginDefinition[];
  readonly #processorCtx: ProcessorContext;
  #initializedPlugins:
    | { instance: ReturnType<MdastPluginDefinition["createOnce"]>; name: string }[]
    | null = null;

  constructor(pluginDefs: MdastPluginDefinition[]) {
    for (const def of pluginDefs) {
      if (!def.name || typeof def.createOnce !== "function") {
        throw new Error(`Invalid plugin: ${JSON.stringify(def.name)}`);
      }
    }
    this.#pluginDefs = pluginDefs;
    this.#processorCtx = new ProcessorContext();
  }

  #getPluginInstances(): { instance: ReturnType<MdastPluginDefinition["createOnce"]>; name: string }[] {
    if (this.#initializedPlugins === null) {
      this.#initializedPlugins = this.#pluginDefs.map((def) => ({
        instance: def.createOnce(this.#processorCtx),
        name: def.name,
      }));
    }
    return this.#initializedPlugins;
  }

  async processBuffer(
    buffer: ArrayBuffer | Uint8Array,
    options: { filename?: string } = {},
  ): Promise<ProcessBufferResult> {
    return runPluginsOnBuffer(buffer, this.#getPluginInstances(), options);
  }

  async processBufferToTree(
    buffer: ArrayBuffer | Uint8Array,
    options: { filename?: string } = {},
  ): Promise<ProcessTreeResult> {
    const result = await this.processBuffer(buffer, options);
    const reader = new MdastReader(result.buffer);
    const tree = materializeTree(reader, result.dataMap);
    return {
      tree,
      dataMap: result.dataMap,
      diagnostics: result.diagnostics,
      mutationCount: result.mutationCount,
    };
  }

  getDiagnostics(): Diagnostic[] {
    return this.#processorCtx.getDiagnostics();
  }
}
