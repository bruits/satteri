import type { Position } from "unist";

// A leading U+FEFF is string content here, not a transport BOM.
const textDecoder = new TextDecoder("utf-8", { ignoreBOM: true });

export function ru16(view: DataView, off: number): number {
  return view.getUint16(off, true);
}

export function ru32(view: DataView, off: number): number {
  return view.getUint32(off, true);
}

// Short ASCII strings avoid TextDecoder overhead; its crossover is around 8–12 bytes.
export function rstr(buf: Uint8Array, off: number, len: number): string {
  if (len === 0) return "";
  if (len <= 8) {
    let ascii = true;
    for (let i = 0; i < len; i++) {
      if (buf[off + i]! > 127) {
        ascii = false;
        break;
      }
    }
    if (ascii) {
      let s = "";
      for (let i = 0; i < len; i++) s += String.fromCharCode(buf[off + i]!);
      return s;
    }
  }
  return textDecoder.decode(buf.subarray(off, off + len));
}

// Unist lines are 1-based; a zero start line marks a synthesized node without a source position.
export function readPosition(view: DataView, off: number): Position | undefined {
  const startLine = ru32(view, off + 8);
  if (startLine === 0) return undefined;
  const startOffset = ru32(view, off);
  return {
    start: { offset: startOffset, line: startLine, column: ru32(view, off + 12) },
    end: { offset: ru32(view, off + 4), line: ru32(view, off + 16), column: ru32(view, off + 20) },
  };
}
