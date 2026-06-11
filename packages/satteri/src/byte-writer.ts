/**
 * Growable little-endian byte buffer shared by the op-stream and command-buffer
 * encoders — the two hottest write paths in the package.
 *
 * Subclasses write single bytes directly through the protected `buf`/`n`
 * fields after an `ensure` for the whole record (one bounds check per record,
 * not per byte, and no per-byte call in unoptimized tiers — CodSpeed's
 * Simulation mode runs too few iterations for V8 to inline method calls).
 * Only the non-trivial logic lives here: growth, u32 writes, and UTF-8
 * string encoding.
 */

const encoder = new TextEncoder();

const EMPTY = new Uint8Array(0);

/** Below this length the inline char-code copy beats `encodeInto`'s call
 *  overhead; above it the native bulk path wins (and skips the ASCII scan).
 *  Measured crossover ~16 bytes (Node 24: 8B 15 vs 41 ns, 16B 37 vs 43,
 *  32B 73 vs 52). */
const INLINE_STR_MAX = 16;

export class ByteWriter {
  // Backing allocates on first write: visitor passes construct buffers that
  // often never receive a byte (read-only plugins).
  protected buf: Uint8Array = EMPTY;
  protected n = 0;
  readonly #initialSize: number;

  constructor(initialSize: number) {
    this.#initialSize = initialSize;
  }

  /** Number of bytes written so far. */
  get length(): number {
    return this.n;
  }

  /** Reset for reuse; the grown buffer is retained so steady state is alloc-free. */
  reset(): void {
    this.n = 0;
  }

  /** View of the bytes written so far (no copy; valid until the next write or reset). */
  take(): Uint8Array {
    return this.n === 0 ? EMPTY : this.buf.subarray(0, this.n);
  }

  /** Release the backing (handed-out views stay intact); next write re-allocates. */
  protected release(): void {
    this.buf = EMPTY;
    this.n = 0;
  }

  /** Grow (doubling) so `extra` more bytes fit; required before unchecked writes. */
  ensure(extra: number): void {
    if (this.n + extra <= this.buf.length) return;
    let size = Math.max(this.#initialSize, this.buf.length * 2);
    while (this.n + extra > size) size *= 2;
    const grown = new Uint8Array(size);
    grown.set(this.buf);
    this.buf = grown;
  }

  /** Write a u32 (LE) at the cursor; caller must have `ensure`d 4 bytes. */
  protected writeU32(v: number): void {
    const buf = this.buf;
    let n = this.n;
    buf[n++] = v & 255;
    buf[n++] = (v >> 8) & 255;
    buf[n++] = (v >> 16) & 255;
    buf[n++] = (v >>> 24) & 255;
    this.n = n;
  }

  /** u32 byte length + UTF-8 bytes (self-ensuring). */
  protected utf8WithU32Len(s: string): void {
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
        // Inline stores, not writeU32: a per-string method call in the
        // unoptimized tier is exactly what this class's header rules out.
        const buf = this.buf;
        let n = this.n;
        buf[n++] = len & 255;
        buf[n++] = (len >> 8) & 255;
        buf[n++] = (len >> 16) & 255;
        buf[n++] = (len >>> 24) & 255;
        for (let i = 0; i < len; i++) buf[n + i] = s.charCodeAt(i);
        this.n = n + len;
        return;
      }
    }

    // Bulk path: encodeInto writes UTF-8 straight into the buffer (no alloc, no
    // per-char loop); backpatch the byte length once it's known.
    const lenPos = this.n;
    this.n += 4;
    const written = encoder.encodeInto(s, this.buf.subarray(this.n)).written;
    this.buf[lenPos] = written & 255;
    this.buf[lenPos + 1] = (written >> 8) & 255;
    this.buf[lenPos + 2] = (written >> 16) & 255;
    this.buf[lenPos + 3] = (written >>> 24) & 255;
    this.n += written;
  }
}
