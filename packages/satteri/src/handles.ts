/**
 * Opaque handles to Rust-owned arenas. The napi-generated `index.d.ts` refers
 * to `MdastHandle`/`HastHandle`/`AnyHandle` as bare, undeclared names; these
 * global declarations give those signatures real types. The kind brand makes
 * passing a hast handle to an mdast entry point (a runtime error in Rust) a
 * compile error in TS.
 */
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

// Module-scope aliases so the globals are importable/re-exportable.
type MdastHandleAlias = MdastHandle;
type HastHandleAlias = HastHandle;
type AnyHandleAlias = AnyHandle;

export type {
  MdastHandleAlias as MdastHandle,
  HastHandleAlias as HastHandle,
  AnyHandleAlias as AnyHandle,
};
