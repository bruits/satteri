// Global handle brands type the bare names emitted by NAPI and prevent mixing MDAST and HAST handles.
declare global {
  interface MdastHandle {
    readonly __satteriHandleKind: "mdast";
  }

  interface HastHandle {
    readonly __satteriHandleKind: "hast";
  }

  /** Handle accepted by kind-agnostic entry points (drop, serialize, …). */
  type AnyHandle = MdastHandle | HastHandle;
}

type MdastHandleAlias = MdastHandle;
type HastHandleAlias = HastHandle;
type AnyHandleAlias = AnyHandle;

export type {
  MdastHandleAlias as MdastHandle,
  HastHandleAlias as HastHandle,
  AnyHandleAlias as AnyHandle,
};
