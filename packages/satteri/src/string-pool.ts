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

  constructor(starts: Uint32Array, shifts: Uint32Array) {
    this.#starts = starts;
    this.#shifts = shifts;
    this.#count = starts.length;
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
    const index = this.#seek(offset);
    const shift = this.#shiftAt(index);
    const start = offset - shift;
    const endByte = offset + len;
    // A ref spanning no multibyte character shifts equally at both ends.
    if (index === this.#count || (this.#starts[index] ?? 0) >= endByte) {
      return text.substring(start, endByte - shift);
    }
    return text.substring(start, endByte - this.#shiftAt(this.#seek(endByte)));
  }
}
