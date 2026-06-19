---
cargo/satteri-pulldown-cmark: patch
cargo/satteri-mdxjs: patch
npm/satteri: patch
---

MDX parse errors now carry a source line and column. Previously, errors in `import`/`export` blocks dropped the position entirely, and errors in `{…}` expressions and JSX attributes were reported as a bare byte offset, so downstream tooling reported an unknown location. JSX attribute and spread expression errors now point at the offending attribute rather than the element's opening `<`.
