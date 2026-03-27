/**
 * End-to-end JS pipeline benchmarks.
 *
 * Covers the full stack from the JS side: parse → HAST binary → HTML.
 * Requires the native Rust module to be built:
 *   cargo build --release -p tryckeri-napi
 *
 * Run with: pnpm bench
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";
import { bench, describe } from "vitest";
import {
  parseToBuffer,
  parseToHastBuffer,
  parseToHtml,
  mdastBufferToHastBuffer,
  hastBufferToHtmlStr,
  compileMdx,
  compileMdxFromBuffer,
} from "../src/index.js";
import { MdastReader } from "../src/mdast-reader.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

const MARKDOWN = readFileSync(new URL("./markdown.md", import.meta.url), "utf8");
const MDX = `import {Chart} from './chart.js'

# Hello, world

Some *emphasis* and **strong** content.

<Chart values={[1, 2, 3]} />

> A blockquote with a [link](https://example.com).

- item one
- item two
- item three
`;

// Pre-computed buffers so intermediate benchmarks measure only their step.
const mdastBuf = parseToBuffer(MARKDOWN);
const hastBuf = mdastBufferToHastBuffer(mdastBuf);

// ---------------------------------------------------------------------------
// Parse benchmarks
// ---------------------------------------------------------------------------

describe("parse", () => {
  bench("parseToBuffer — Markdown → MDAST binary", () => {
    parseToBuffer(MARKDOWN);
  });

  bench("parseToHastBuffer — Markdown → HAST binary (combined Rust path)", () => {
    parseToHastBuffer(MARKDOWN);
  });
});

// ---------------------------------------------------------------------------
// HAST / HTML benchmarks
// ---------------------------------------------------------------------------

describe("hast", () => {
  bench("mdastBufferToHastBuffer — MDAST binary → HAST binary", () => {
    mdastBufferToHastBuffer(mdastBuf);
  });

  bench("hastBufferToHtmlStr — HAST binary → HTML string", () => {
    hastBufferToHtmlStr(hastBuf);
  });

  bench("full pipeline — parseToBuffer → mdastBufferToHastBuffer → hastBufferToHtmlStr", () => {
    const buf = parseToBuffer(MARKDOWN);
    const hast = mdastBufferToHastBuffer(buf);
    hastBufferToHtmlStr(hast);
  });
});

// ---------------------------------------------------------------------------
// MDAST reader benchmark (JS-only, no native call)
// ---------------------------------------------------------------------------

describe("mdast-reader", () => {
  bench("MdastReader — walk all nodes from pre-parsed buffer", () => {
    const reader = new MdastReader(mdastBuf);
    for (let i = 0; i < reader.nodeCount; i++) {
      reader.getNode(i);
    }
  });
});

// ---------------------------------------------------------------------------
// Full pipeline benchmark
// ---------------------------------------------------------------------------

describe("e2e", () => {
  bench("no plugins — parseToHtml (pure Rust, single NAPI call)", () => {
    parseToHtml(MARKDOWN);
  });
});

// ---------------------------------------------------------------------------
// MDX benchmarks
// ---------------------------------------------------------------------------

// MDX compilation requires the native module to be rebuilt after adding compileMdx.
// Run: cargo build --release -p tryckeri-napi
try {
  compileMdx("# test"); // probe — throws if not yet built
  // Pre-parse MDX with MDX constructs enabled so compileMdxFromBuffer has a valid buffer.
  // (parseToBuffer uses default ParseOptions which don't enable MDX constructs, so we
  // use compileMdx itself as the parse step for that bench.)
  const mdxBuf = parseToBuffer(MDX); // MDAST binary (no MDX constructs — intentional: measures the buffer path perf)
  describe("mdx", () => {
    bench("compileMdx — MDX source → JavaScript (parse + compile)", () => {
      compileMdx(MDX);
    });
    bench("compileMdxFromBuffer — pre-parsed MDAST binary → JavaScript", () => {
      compileMdxFromBuffer(mdxBuf);
    });
  });
} catch {
  // compileMdx not available in the current native binary; skip.
}

// ---------------------------------------------------------------------------
// Async pipeline benchmarks (measure async overhead)
// ---------------------------------------------------------------------------

import {
  compileMarkdownToHtml,
  compileMdxToJs,
  defineHastPlugin,
} from "../src/index.js";
import type { HastNode } from "../src/hast-materializer.js";
import type { HastVisitorContext } from "../src/hast-visitor.js";

describe("async pipeline", () => {
  bench("compileMarkdownToHtml — no plugins (async overhead)", async () => {
    await compileMarkdownToHtml(MARKDOWN);
  });

  bench("compileMarkdownToHtml — sync HAST plugin", async () => {
    const plugin = defineHastPlugin({
      name: "sync-noop",
      createOnce: () => ({
        element() {
          // no-op sync
        },
      }),
    });
    await compileMarkdownToHtml(MARKDOWN, { hastPlugins: [plugin] });
  });

  bench("compileMarkdownToHtml — async HAST plugin", async () => {
    const plugin = defineHastPlugin({
      name: "async-noop",
      createOnce: () => ({
        async element(_node: HastNode, _ctx: HastVisitorContext) {
          // no-op async (returns a resolved promise)
        },
      }),
    });
    await compileMarkdownToHtml(MARKDOWN, { hastPlugins: [plugin] });
  });
});
