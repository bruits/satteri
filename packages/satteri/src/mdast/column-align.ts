/** Decode a table column-alignment byte — shared by the generated walk decoder
 *  and the snapshot reader so the mapping lives in one place. */
const ALIGN_NAMES: readonly (string | null)[] = [null, "left", "right", "center"];

export function decodeColumnAlign(byte: number): string | null {
  return ALIGN_NAMES[byte] ?? null;
}
