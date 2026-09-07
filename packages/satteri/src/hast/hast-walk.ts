import { parseEsm as napiParseEsm, parseExpression as napiParseExpression } from "#binding";
import type { Program } from "estree-jsx";
import type { AnyHandle } from "../handles.js";
import {
  LazyChildResolver,
  registerEpochCacheSlot,
  type EpochCache,
} from "../lazy-child-resolver.js";
import type { Position } from "../types.js";
import { crossPipelineForeign, FOREIGN_REF, type NodeRefs } from "../visitor-shared.js";
import { readPosition, rstr } from "../wire-read.js";
import { HastChildStub } from "./child-stub.js";
import { TYPE_NAMES } from "./generated/node-types.js";
import {
  decodeWalkElementProps,
  readWalkElementTag,
  readWalkHastValue,
  readWalkMdxJsx,
  walkElementPropCount,
  walkElementPropsAt,
} from "./generated/walk-decode.js";
import { materializeHastNode, type HastNode } from "./hast-materializer.js";
import {
  HAST_COMMENT,
  HAST_ELEMENT,
  HAST_MDX_ESM,
  HAST_MDX_FLOW_EXPRESSION,
  HAST_MDX_JSX_ELEMENT,
  HAST_MDX_JSX_TEXT_ELEMENT,
  HAST_MDX_TEXT_EXPRESSION,
  HAST_RAW,
  HAST_ROOT,
  HAST_TEXT,
  HastReader,
} from "./hast-reader.js";

type NapiParseFn = (source: string) => string | null;

function attachParseExpression(node: HastNode, parseFn: NapiParseFn): void {
  Object.defineProperty(node, "parseExpression", {
    value(): Program | null {
      const value = (this as { value?: string }).value;
      if (typeof value !== "string") return null;
      const json = parseFn(value);
      if (json == null) return null;
      return JSON.parse(json) as Program;
    },
    writable: false,
    enumerable: false,
    configurable: true,
  });
}

// Spread copies must be treated as new content, not as references to the original arena node.
export function getNodeId(node: HastNode, refs: NodeRefs): number | undefined {
  if (node instanceof WalkElement) return node._refs === refs ? node._nid : FOREIGN_REF;
  if (node instanceof HastChildStub) return node._refs === refs ? node._id : FOREIGN_REF;
  const id = refs.get(node);
  if (id !== undefined) return id;
  const d = Object.getOwnPropertyDescriptor(node, "_nodeId");
  if (d !== undefined && !d.enumerable) return FOREIGN_REF;
  return crossPipelineForeign(node);
}

function readChildStubs(
  view: DataView,
  buf: Uint8Array,
  idsPos: number,
  typesPos: number,
  count: number,
  resolver: HastLazyChildResolver,
): HastNode[] {
  // With a hot snapshot a stub's deferral buys nothing; real nodes skip its per-field getters.
  if (resolver.hasHotSnapshot()) {
    const nodes: HastNode[] = new Array(count);
    for (let i = 0; i < count; i++) {
      nodes[i] = resolver.materializeOne(view.getUint32(idsPos + i * 4, true));
    }
    return nodes;
  }
  const stubs: HastNode[] = new Array(count);
  for (let i = 0; i < count; i++) {
    stubs[i] = new HastChildStub(
      resolver,
      view.getUint32(idsPos + i * 4, true),
      buf[typesPos + i]!,
    ) as unknown as HastNode;
  }
  return stubs;
}

type HastProperties = Record<string, string | number | boolean | (string | number)[]>;

// One shared wire reference per element reduces constructor stores.
export interface WalkWire {
  view: DataView;
  buf: Uint8Array;
  resolver: HastLazyChildResolver;
}

// Shared getters need a static block to access private wire fields.
let WALK_PROPS_DESC!: PropertyDescriptor;
let WALK_CHILDREN_DESC!: PropertyDescriptor;

// Private wire fields preserve spread identity without the GC cost of a WeakMap entry per element.
class WalkElement {
  readonly type = "element" as const;
  tagName: string;
  declare properties: HastProperties;
  declare position?: Position;
  declare data?: Record<string, unknown>;
  declare children?: HastNode[];

  readonly #nodeId: number;
  #wire: WalkWire;
  #propsPos: number;
  #childIdsPos: number;
  #childTypesPos: number;
  #childCount: number;

  constructor(
    tagName: string,
    nodeId: number,
    wire: WalkWire,
    propsPos: number,
    propCount: number,
    childIdsPos: number,
    childTypesPos: number,
    childCount: number,
  ) {
    this.tagName = tagName;
    this.#nodeId = nodeId;
    this.#wire = wire;
    this.#propsPos = propsPos;
    this.#childIdsPos = childIdsPos;
    this.#childTypesPos = childTypesPos;
    this.#childCount = childCount;
    if (propCount === 0) {
      this.properties = {};
    } else {
      Object.defineProperty(this, "properties", WALK_PROPS_DESC);
    }
    if (childCount === 0) {
      this.children = [];
    } else {
      Object.defineProperty(this, "children", WALK_CHILDREN_DESC);
    }
  }

  /** @internal */
  get _nid(): number {
    return this.#nodeId;
  }

  /** @internal */
  get _refs(): NodeRefs {
    return this.#wire.resolver.refs;
  }

  static {
    WALK_PROPS_DESC = {
      enumerable: true,
      configurable: true,
      get(this: WalkElement): HastProperties {
        const w = this.#wire;
        const val = decodeWalkElementProps(w.view, w.buf, this.#propsPos);
        Object.defineProperty(this, "properties", {
          value: val,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        return val;
      },
    };
    WALK_CHILDREN_DESC = {
      enumerable: true,
      configurable: true,
      get(this: WalkElement): HastNode[] {
        const w = this.#wire;
        const val = readChildStubs(
          w.view,
          w.buf,
          this.#childIdsPos,
          this.#childTypesPos,
          this.#childCount,
          w.resolver,
        );
        Object.defineProperty(this, "children", {
          value: val,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        return val;
      },
    };
  }
}

function readElementFromBinary(
  wire: WalkWire,
  offset: number,
  nodeId: number,
  position: Position | undefined,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  data: Record<string, unknown> | null,
): HastNode {
  // Visitors almost always read tagName, so decode it eagerly.
  const tagName = readWalkElementTag(wire.view, wire.buf, offset);
  const pos = walkElementPropsAt(wire.view, offset);
  const propCount = walkElementPropCount(wire.view, pos);
  const node = new WalkElement(
    tagName,
    nodeId,
    wire,
    pos,
    propCount,
    childIdsPos,
    childTypesPos,
    childCount,
  );
  if (position !== undefined) node.position = position;
  if (data !== null) node.data = data;
  return node as unknown as HastNode;
}

function readTextFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  position: Position | undefined,
  data: Record<string, unknown> | null,
  refs: NodeRefs,
): HastNode {
  const value = readWalkHastValue(view, buf, offset, nodeType);
  const base: Record<string, unknown> = {
    type: TYPE_NAMES[nodeType] ?? `unknown(${nodeType})`,
    value,
  };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  const node = base as unknown as HastNode;
  refs.set(node, nodeId);
  if (nodeType === HAST_MDX_FLOW_EXPRESSION || nodeType === HAST_MDX_TEXT_EXPRESSION) {
    attachParseExpression(node, napiParseExpression);
  } else if (nodeType === HAST_MDX_ESM) {
    attachParseExpression(node, napiParseEsm);
  }
  return node;
}

function readMdxJsxFromBinary(
  view: DataView,
  buf: Uint8Array,
  offset: number,
  nodeId: number,
  nodeType: number,
  resolver: HastLazyChildResolver,
  position: Position | undefined,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  data: Record<string, unknown> | null,
): HastNode {
  const { name, attributes } = readWalkMdxJsx(view, buf, offset);

  const typeName = nodeType === HAST_MDX_JSX_ELEMENT ? "mdxJsxFlowElement" : "mdxJsxTextElement";
  const base: Record<string, unknown> = { type: typeName, name, attributes };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  resolver.refs.set(base, nodeId);
  makeLazyChildren(base, view, buf, childIdsPos, childTypesPos, childCount, resolver);
  return base as unknown as HastNode;
}

export function readMatchedNode(
  wire: WalkWire,
  offset: number,
  nodeId: number,
  nodeType: number,
): HastNode {
  const { view, buf, resolver } = wire;
  let pos = offset;

  const dataLen = view.getUint32(pos, true);
  pos += 4;
  let data: Record<string, unknown> | null = null;
  if (dataLen > 0) {
    const jsonStr = rstr(buf, pos, dataLen);
    try {
      data = JSON.parse(jsonStr) as Record<string, unknown>;
    } catch (err) {
      if (process.env.NODE_ENV !== "production") {
        console.warn(`readMatchedNode: malformed node_data for nodeId=${nodeId}`, err);
      }
    }
    pos += dataLen;
  }

  const position = readPosition(view, pos);
  pos += 24;

  const childCount = view.getUint32(pos, true);
  pos += 4;
  // Ids/types decode lazily with `.children` — most matched nodes never read them.
  const childIdsPos = pos;
  pos += childCount * 4;
  const childTypesPos = pos;
  pos += childCount;

  if (nodeType === HAST_ELEMENT) {
    return readElementFromBinary(
      wire,
      pos,
      nodeId,
      position,
      childIdsPos,
      childTypesPos,
      childCount,
      data,
    );
  } else if (
    nodeType === HAST_TEXT ||
    nodeType === HAST_COMMENT ||
    nodeType === HAST_RAW ||
    nodeType === HAST_MDX_FLOW_EXPRESSION ||
    nodeType === HAST_MDX_TEXT_EXPRESSION ||
    nodeType === HAST_MDX_ESM
  ) {
    return readTextFromBinary(view, buf, pos, nodeId, nodeType, position, data, resolver.refs);
  } else if (nodeType === HAST_MDX_JSX_ELEMENT || nodeType === HAST_MDX_JSX_TEXT_ELEMENT) {
    return readMdxJsxFromBinary(
      view,
      buf,
      pos,
      nodeId,
      nodeType,
      resolver,
      position,
      childIdsPos,
      childTypesPos,
      childCount,
      data,
    );
  }
  const base: Record<string, unknown> = { type: TYPE_NAMES[nodeType] ?? `unknown(${nodeType})` };
  if (position !== undefined) base.position = position;
  if (data !== null) base.data = data;
  if (nodeType === HAST_ROOT) {
    if (childCount > 0) {
      makeLazyChildren(base, view, buf, childIdsPos, childTypesPos, childCount, resolver);
    } else {
      base.children = [];
    }
  }
  const node = base as unknown as HastNode;
  resolver.refs.set(node, nodeId);
  return node;
}

const HAST_EPOCH_CACHE = registerEpochCacheSlot(new WeakMap<AnyHandle, EpochCache<HastReader>>());

export class HastLazyChildResolver extends LazyChildResolver<HastReader, HastNode> {
  protected override cacheSlot() {
    return HAST_EPOCH_CACHE;
  }

  protected override createReader(wire: Uint8Array): HastReader {
    return new HastReader(wire);
  }

  protected override materializeNode(reader: HastReader, nodeId: number, refs: NodeRefs): HastNode {
    return materializeHastNode(reader, nodeId, true, refs);
  }

  protected override readParentId(reader: HastReader, nodeId: number): number {
    return reader.getParentId(nodeId);
  }

  protected override readChildIds(reader: HastReader, nodeId: number): number[] {
    return reader.getChildIds(nodeId);
  }
}

// Own enumerable getters preserve children in spreads; closures avoid costly per-node wire slots.
function makeLazyChildren(
  node: object,
  view: DataView,
  buf: Uint8Array,
  childIdsPos: number,
  childTypesPos: number,
  childCount: number,
  resolver: HastLazyChildResolver,
): void {
  Object.defineProperty(node, "children", {
    get(this: object): HastNode[] {
      const val = readChildStubs(view, buf, childIdsPos, childTypesPos, childCount, resolver);
      Object.defineProperty(this, "children", {
        value: val,
        writable: true,
        enumerable: true,
        configurable: true,
      });
      return val;
    },
    enumerable: true,
    configurable: true,
  });
}
