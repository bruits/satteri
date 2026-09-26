import { FOREIGN_REF, type NodeRefs } from "./visitor-shared.js";

class PlainNode {
  declare type: string;
  constructor(type: string) {
    return { type };
  }
}

// Private identity avoids a WeakMap insertion for every matched node. A derived constructor
// installs its fields on the plain object returned by super(), preserving plain-node consumers
// without a costly prototype change. Spreads and structured clones never copy this identity.
export class VisitorNode extends PlainNode {
  [key: string]: unknown;
  readonly #refs: NodeRefs;
  readonly #id: number;

  constructor(type: string, refs: NodeRefs, id: number) {
    super(type);
    this.#refs = refs;
    this.#id = id;
  }

  static getNodeId(node: object, refs: NodeRefs): number | undefined {
    if (!(#refs in node)) return undefined;
    return node.#refs === refs ? node.#id : FOREIGN_REF;
  }
}
