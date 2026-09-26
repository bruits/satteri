// Write bytes inline after reserving each record to avoid per-byte calls and bounds checks.

const encoder = new TextEncoder();

const EMPTY = new Uint8Array(0);

// Below 16 bytes, inline ASCII writes beat encodeInto call overhead.
const INLINE_STR_MAX = 16;

export class ByteWriter {
  // Read-only plugin passes should allocate no backing buffer.
  protected buf: Uint8Array = EMPTY;
  protected n = 0;
  readonly #initialSize: number;

  constructor(initialSize: number) {
    this.#initialSize = initialSize;
  }

  get length(): number {
    return this.n;
  }

  get capacity(): number {
    return this.buf.length;
  }

  // Retain capacity so repeated plugin passes can reuse the buffer without allocating.
  reset(): void {
    this.n = 0;
  }

  /** View of the bytes written so far (no copy; valid until the next write or reset). */
  take(): Uint8Array {
    return this.n === 0 ? EMPTY : this.buf.subarray(0, this.n);
  }

  protected release(): void {
    this.buf = EMPTY;
    this.n = 0;
  }

  // Reserve capacity before unchecked writes.
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

  protected utf8WithU32Len(s: string): void {
    const len = s.length;
    this.ensure(4 + len * 3); // worst-case UTF-8 is 3 bytes per UTF-16 unit

    if (len <= INLINE_STR_MAX) {
      let ascii = true;
      for (let i = 0; i < len; i++) {
        if (s.charCodeAt(i) > 127) {
          ascii = false;
          break;
        }
      }
      if (ascii) {
        // Inline stores avoid per-string method calls before V8 has optimized this path.
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

    const lenPos = this.n;
    this.n += 4;
    const written = encoder.encodeInto(s, this.buf.subarray(this.n)).written;
    this.patchU32(lenPos, written);
    this.n += written;
  }

  protected patchU32(pos: number, v: number): void {
    const buf = this.buf;
    buf[pos] = v & 255;
    buf[pos + 1] = (v >> 8) & 255;
    buf[pos + 2] = (v >> 16) & 255;
    buf[pos + 3] = (v >>> 24) & 255;
  }
}
