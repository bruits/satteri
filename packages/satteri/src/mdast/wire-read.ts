/**
 * Little-endian primitives for reading the walk/snapshot wire buffers. Shared by
 * the hand-written decoders and the generated layout decoder so both interpret
 * the bytes identically.
 */

const textDecoder = new TextDecoder("utf-8");

/** Read a u16 (LE) at `off`. */
export function ru16(view: DataView, off: number): number {
  return view.getUint16(off, true);
}

/** Read a u32 (LE) at `off`. */
export function ru32(view: DataView, off: number): number {
  return view.getUint32(off, true);
}

/** Read a UTF-8 string of `len` bytes at `off`. */
export function rstr(buf: Uint8Array, off: number, len: number): string {
  return len === 0 ? "" : textDecoder.decode(buf.subarray(off, off + len));
}
