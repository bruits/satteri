const NO_WORDS = new Uint32Array(0);

/**
 * Byte-offset to UTF-16-offset remap for a wire string pool holding multibyte
 * sequences, so the once-decoded pool can still be sliced with `substring`
 * instead of a `TextDecoder` call per string.
 */
export class PoolOffsets {
  readonly #starts: Uint32Array;
  readonly #shifts: Uint32Array;
  readonly #count: number;
  #hint = 0;

  /** `extraBytes` is the pool's byte length minus its UTF-16 length, which
   *  bounds the multibyte character count: each one costs at least one byte. */
  constructor(bytes: Uint8Array, extraBytes: number) {
    const starts = new Uint32Array(extraBytes);
    const shifts = new Uint32Array(extraBytes);
    const n = bytes.length;
    const pad = (4 - (bytes.byteOffset & 3)) & 3;
    const wordCount = n > pad ? (n - pad) >> 2 : 0;
    // A pool shorter than its alignment padding would put the view past the buffer.
    const words =
      wordCount === 0 ? NO_WORDS : new Uint32Array(bytes.buffer, bytes.byteOffset + pad, wordCount);
    let count = 0;
    let shift = 0;
    let i = 0;
    for (;;) {
      // `i` is always at a character boundary, so a word with no high bits set
      // cannot hide the start of one and the whole word is skippable.
      if (i >= pad && ((i - pad) & 3) === 0) {
        let w = (i - pad) >> 2;
        while (w < wordCount && ((words[w] ?? 0) & 0x80808080) === 0) w++;
        i = pad + (w << 2);
      }
      if (i >= n) break;
      const lead = bytes[i] ?? 0;
      if (lead < 0x80) {
        i++;
        continue;
      }
      starts[count] = i;
      if (lead < 0xe0) {
        shift += 1;
        i += 2;
      } else if (lead < 0xf0) {
        shift += 2;
        i += 3;
      } else {
        shift += 2;
        i += 4;
      }
      shifts[count] = shift;
      count++;
    }
    this.#starts = starts;
    this.#shifts = shifts;
    this.#count = count;
  }

  /** How many multibyte characters start strictly before `byteOffset`. */
  #seek(byteOffset: number): number {
    const starts = this.#starts;
    const count = this.#count;
    const hint = this.#hint;
    if (
      (hint === count || (starts[hint] ?? 0) >= byteOffset) &&
      (hint === 0 || (starts[hint - 1] ?? 0) < byteOffset)
    ) {
      return hint;
    }
    let lo = 0;
    let hi = count;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if ((starts[mid] ?? 0) < byteOffset) lo = mid + 1;
      else hi = mid;
    }
    this.#hint = lo;
    return lo;
  }

  #shiftAt(index: number): number {
    return index === 0 ? 0 : (this.#shifts[index - 1] ?? 0);
  }

  /** Both ends of a string ref sit on character boundaries, so the remap is exact. */
  slice(text: string, offset: number, len: number): string {
    const start = offset - this.#shiftAt(this.#seek(offset));
    const endByte = offset + len;
    return text.substring(start, endByte - this.#shiftAt(this.#seek(endByte)));
  }
}
