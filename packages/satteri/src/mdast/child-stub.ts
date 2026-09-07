import {
  flatByTag,
  installStubDescriptors,
  stubDescriptors,
  type StubDescriptorEntry,
} from "../child-stub.js";
import type { LazyChildResolver } from "../lazy-child-resolver.js";
import type { MdastNode } from "../types.js";
import type { MdastReader } from "./mdast-reader.js";
import { MDAST_LAYOUT_KEYS } from "./generated/layout.js";
import { NAME_TO_TYPE, TYPE_NAMES } from "./generated/node-types.js";
import { LEAF_TYPES } from "./mdast-materializer.js";
import type { NodeRefs } from "../visitor-shared.js";

type MdastResolver = LazyChildResolver<MdastReader, MdastNode>;

const N = NAME_TO_TYPE;

const HAND_WRITTEN_FIELDS: Readonly<Record<number, readonly string[]>> = {
  [N.list!]: ["ordered", "start", "spread"],
  [N.listItem!]: ["spread", "checked"],
  [N.descriptionDetails!]: ["spread"],
  [N.table!]: ["align"],
  [N.containerDirective!]: ["name", "attributes"],
  [N.leafDirective!]: ["name", "attributes"],
  [N.textDirective!]: ["name", "attributes"],
  [N.mdxJsxFlowElement!]: ["name", "attributes"],
  [N.mdxJsxTextElement!]: ["name", "attributes"],
};

const TYPE_NAME_BY_TAG = flatByTag(TYPE_NAMES);

// Custom nodes expose their stored name as type, without a separate name field.
const MDAST_CUSTOM = NAME_TO_TYPE.custom!;

const MDAST_STUB_DESCRIPTORS: (readonly StubDescriptorEntry[] | undefined)[] = [];
for (const tag of Object.keys(TYPE_NAMES)) {
  const nodeType = Number(tag);
  const fields =
    nodeType === MDAST_CUSTOM
      ? ["value"]
      : [...(MDAST_LAYOUT_KEYS[nodeType] ?? HAND_WRITTEN_FIELDS[nodeType] ?? [])];
  // `custom` gets its own `children` getter: leafness is per node, not per type.
  if (nodeType !== MDAST_CUSTOM && !LEAF_TYPES.has(nodeType)) fields.push("children");
  MDAST_STUB_DESCRIPTORS[nodeType] = stubDescriptors(fields);
}

const FALLBACK_DESCRIPTORS = stubDescriptors([]);

// Custom type names live in the arena, so reading type must remain lazy too.
export class MdastChildStub {
  _resolver: MdastResolver;
  _id: number;
  type!: string;

  constructor(resolver: MdastResolver, id: number, nodeType: number) {
    this._resolver = resolver;
    this._id = id;
    if (nodeType === MDAST_CUSTOM) {
      installLazyCustomType(this);
      installLazyCustomChildren(this);
    } else {
      this.type = TYPE_NAME_BY_TAG[nodeType] ?? `unknown(${nodeType})`;
    }
    installStubDescriptors(this, MDAST_STUB_DESCRIPTORS[nodeType] ?? FALLBACK_DESCRIPTORS);
  }

  /** @internal */
  get _refs(): NodeRefs {
    return this._resolver.refs;
  }
}

// A custom node’s leafness is unknown until materialization; leaf stubs must lose their children field.
function installLazyCustomChildren(stub: MdastChildStub): void {
  Object.defineProperty(stub, "children", {
    get(this: MdastChildStub): MdastNode[] | undefined {
      const real = this._resolver.materializeOne(this._id) as { children?: MdastNode[] };
      const value = real.children;
      if (value === undefined) {
        delete (this as { children?: MdastNode[] }).children;
        return undefined;
      }
      Object.defineProperty(this, "children", {
        value,
        writable: false,
        enumerable: true,
        configurable: false,
      });
      return value;
    },
    enumerable: true,
    configurable: true,
  });
}

function installLazyCustomType(stub: MdastChildStub): void {
  Object.defineProperty(stub, "type", {
    get(this: MdastChildStub): string {
      const value = this._resolver.materializeOne(this._id).type;
      Object.defineProperty(this, "type", {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
      });
      return value;
    },
    enumerable: true,
    configurable: true,
  });
}
