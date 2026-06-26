---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

Fixes JSX nested in an MDX attribute expression (e.g. `prop={<p>hi</p>}` or `title={<>x</>}`) being emitted as raw, un-lowered JSX, which produced invalid JavaScript. Also fixes quotes and apostrophes in such JSX text (e.g. `prop={<p>Inc.'s "best" tool</p>}`) being mis-scanned as JS string literals and causing a parse error — the expression scanner now consumes a JSX element's children as text.
