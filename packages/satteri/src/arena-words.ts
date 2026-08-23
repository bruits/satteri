import { FIELD } from "./generated/arena-layout.js";

/** `FIELD` byte offsets as u32-word indices, for readers indexing a `Uint32Array` view of the wire. */
export const W_PARENT = FIELD.parent >> 2;
export const W_START_OFFSET = FIELD.start_offset >> 2;
export const W_CHILDREN_START = FIELD.children_start >> 2;
export const W_CHILDREN_COUNT = FIELD.children_count >> 2;
export const W_DATA_OFFSET = FIELD.data_offset >> 2;
export const W_DATA_LEN = FIELD.data_len >> 2;
