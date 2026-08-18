import { test, expect } from "vitest";
import {
  CommandBuffer,
  COMMAND_BUFFER_RETAIN_MAX,
  acquireCommandBuffer,
  releaseCommandBuffer,
} from "../src/command-buffer.js";

// The pool caps at 8 entries; acquiring that many guarantees it is empty.
function drainPool(): void {
  for (let i = 0; i < 8; i++) acquireCommandBuffer();
}

function grownPastRetainMax(): CommandBuffer {
  const buf = acquireCommandBuffer();
  buf.setProperty(1, "k", "x".repeat(COMMAND_BUFFER_RETAIN_MAX));
  expect(buf.capacity).toBeGreaterThan(COMMAND_BUFFER_RETAIN_MAX);
  return buf;
}

test("an oversized buffer is dropped on release, not re-served by the pool", () => {
  drainPool();
  const big = grownPastRetainMax();
  releaseCommandBuffer(big);
  expect(acquireCommandBuffer()).not.toBe(big);
});

test("a normal-size buffer keeps pooling", () => {
  drainPool();
  const buf = acquireCommandBuffer();
  buf.setProperty(1, "k", "v");
  releaseCommandBuffer(buf);
  const reacquired = acquireCommandBuffer();
  expect(reacquired).toBe(buf);
  expect(reacquired.length).toBe(0);
});
