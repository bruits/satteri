// Rust uses this sentinel for spaces inside MDX expressions; every JS read path must restore them.
const PHANTOM_SPACE = "\uF002";

export function restorePhantomSpaces(value: string): string {
  return value.includes(PHANTOM_SPACE) ? value.replaceAll(PHANTOM_SPACE, " ") : value;
}
