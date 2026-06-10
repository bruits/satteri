/**
 * Growable little-endian byte writer shared by the op-stream and command-buffer
 * encoders — the two hottest write paths in the package.
 *
 * Numeric writes (`u8`/`u32`/`bytes`) are unchecked: callers `ensure` the full
 * record up front so each record pays one bounds check, not one per byte.
 * String writes self-ensure because their byte length is data-dependent.
 */

const encoder = new TextEncoder();

/** Below this length the inline char-code copy beats `encodeInto`'s call
 *  overhead; above it the native bulk path wins (and skips the ASCII scan).
 *  Measured crossover ~16 bytes (Node 24: 8B 15 vs 41 ns, 16B 37 vs 43,
 *  32B 73 vs 52). */
const INLINE_STR_MAX = 16;

export class ByteWriter {
  #buf: Uint8Array;
  #n = 0;

  constructor(initialSize: number) {
    this.#buf = new Uint8Array(initialSize);
  }

  /** Number of bytes written so far. */
  get length(): number {
    return this.#n;
  }

  /** Reset for reuse; the grown buffer is retained so steady state is alloc-free. */
  reset(): void {
    this.#n = 0;
  }

  /** View of the bytes written so far (no copy; valid until the next write or reset). */
  take(): Uint8Array {
    return this.#buf.subarray(0, this.#n);
  }

  /** Grow (doubling) so `extra` more bytes fit; required before unchecked writes. */
  ensure(extra: number): void {
    if (this.#n + extra <= this.#buf.length) return;
    let size = this.#buf.length * 2;
    while (this.#n + extra > size) size *= 2;
    const grown = new Uint8Array(size);
    grown.set(this.#buf);
    this.#buf = grown;
  }

  u8(v: number): void {
    this.#buf[this.#n++] = v & 255;
  }

  u32(v: number): void {
    this.#buf[this.#n++] = v & 255;
    this.#buf[this.#n++] = (v >> 8) & 255;
    this.#buf[this.#n++] = (v >> 16) & 255;
    this.#buf[this.#n++] = (v >>> 24) & 255;
  }

  bytes(src: Uint8Array): void {
    this.#buf.set(src, this.#n);
    this.#n += src.length;
  }

  /** u32 byte length + UTF-8 bytes (self-ensuring). */
  utf8WithU32Len(s: string): void {
    const len = s.length;
    this.ensure(4 + len * 3); // worst-case UTF-8 is 3 bytes per UTF-16 unit

    // Short strings: inline char copy (cheaper than a native call), guarded by a
    // quick ASCII scan. Anything longer — or non-ASCII — goes to encodeInto.
    if (len <= INLINE_STR_MAX) {
      let ascii = true;
      for (let i = 0; i < len; i++) {
        if (s.charCodeAt(i) > 127) {
          ascii = false;
          break;
        }
      }
      if (ascii) {
        this.u32(len);
        const buf = this.#buf;
        const n = this.#n;
        for (let i = 0; i < len; i++) buf[n + i] = s.charCodeAt(i);
        this.#n = n + len;
        return;
      }
    }

    // Bulk path: encodeInto writes UTF-8 straight into the buffer (no alloc, no
    // per-char loop); backpatch the byte length once it's known.
    const lenPos = this.#n;
    this.#n += 4;
    const written = encoder.encodeInto(s, this.#buf.subarray(this.#n)).written;
    this.#buf[lenPos] = written & 255;
    this.#buf[lenPos + 1] = (written >> 8) & 255;
    this.#buf[lenPos + 2] = (written >> 16) & 255;
    this.#buf[lenPos + 3] = (written >>> 24) & 255;
    this.#n += written;
  }
}
