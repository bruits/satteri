/**
 * Byte-offset to UTF-16-offset remap for a wire string pool holding multibyte
 * sequences, so the once-decoded pool can still be sliced with `substring`
 * instead of a `TextDecoder` call per string.
 */
export class PoolOffsets {
  readonly #starts: Uint32Array;
  readonly #shifts: Uint32Array;
  readonly #count: number;
  readonly #first: number;
  #hint = 0;

  constructor(starts: Uint32Array, shifts: Uint32Array) {
    this.#starts = starts;
    this.#shifts = shifts;
    this.#count = starts.length;
    this.#first = starts.length > 0 ? (starts[0] ?? 0) : 0xffffffff;
  }

  /** How many multibyte characters start strictly before `byteOffset`. */
  #seek(byteOffset: number): number {
    const starts = this.#starts;
    const count = this.#count;
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

  /** Both ends of a string ref sit on character boundaries, so the remap is exact. */
  slice(text: string, offset: number, len: number): string {
    const endByte = offset + len;
    if (endByte <= this.#first) return text.substring(offset, endByte);
    const starts = this.#starts;
    const shifts = this.#shifts;
    const count = this.#count;
    const hint = this.#hint;
    const index =
      (hint === count || (starts[hint] ?? 0) >= offset) &&
      (hint === 0 || (starts[hint - 1] ?? 0) < offset)
        ? hint
        : this.#seek(offset);
    const shift = index === 0 ? 0 : (shifts[index - 1] ?? 0);
    const start = offset - shift;
    // A ref spanning no multibyte character shifts equally at both ends.
    if (index === count || (starts[index] ?? 0) >= endByte) {
      return text.substring(start, endByte - shift);
    }
    const endIndex = this.#seek(endByte);
    const endShift = endIndex === 0 ? 0 : (shifts[endIndex - 1] ?? 0);
    return text.substring(start, endByte - endShift);
  }
}
