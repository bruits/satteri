import { HastReader, materializeHastTree } from "satteri/hast";

export function compileMdxToHast(source, compile) {
  try {
    return materializeHastTree(new HastReader(compile(source)));
  } catch (error) {
    if (error instanceof Error) throw error;
    throw new Error(String(error));
  }
}
