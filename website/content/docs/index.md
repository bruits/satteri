---
title: "Prologue"
section: "getting-started"
---

Sätteri is a Markdown processing pipeline. The parser and the AST live in Rust; the plugin layer lives in JavaScript. You write your transforms in TypeScript and pay close to nothing for the language boundary.

The JavaScript Markdown ecosystem has rich plugins but slow parsers. The Rust Markdown ecosystem is the reverse. Sätteri sits between them.

## Features

- A native parser built on `pulldown-cmark`, with configurable support for GFM, frontmatter, math, and remark-directive containers
- MDX as a separate parser entry point, with the same plugin model
- A typed plugin API exposed to JavaScript through napi-rs
- Native binaries for macOS, Linux, and Windows; a WASI fallback for browsers and edge runtimes

## What it isn't

Sätteri isn't a drop-in replacement for unified. The AST shapes are the same (HAST and MDAST), so the plugin model will look familiar to anyone who's worked with remark or rehype, but Sätteri's plugin API isn't compatible. Existing remark/rehype plugins won't run unmodified.

If you need to reuse those plugins as-is, stick with unified.

## Next

Read [Installation](/docs/installation/), then [Quick start](/docs/quick-start/).

Questions or feedback? [Join the Discord](/chat/).
