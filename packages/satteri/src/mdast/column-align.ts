const ALIGN_NAMES = [null, "left", "right", "center"] as const;

export function decodeColumnAlign(byte: number) {
  return ALIGN_NAMES[byte] ?? null;
}
