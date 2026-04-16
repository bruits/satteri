# satteri-pulldown-cmark

## 0.2.2 — 2026-04-16

### Patch changes

- [6f9f66f](https://github.com/bruits/satteri/commit/6f9f66fa75722c0b58f50783b5ac85fefd53a157) Fixed JSX inside MDX expression bodies, JSX inside `.map()` callbacks or other expressions is now compiled to `_jsx()` calls instead of being dropped or emitted as raw JSX — Thanks @Princesseuh!

## 0.2.1 — 2026-04-16

### Patch changes

- [ef20299](https://github.com/bruits/satteri/commit/ef202996675d5e45548e34bef49da906c28a30e9) Fixed `code.value` in the MDAST tree including a trailing newline for well-formed fenced code blocks, which diverged from `remark-parse`. MDAST plugins inspecting `node.value` now see the same bytes as remark. — Thanks @Princesseuh!
- Updated dependencies: satteri-ast (Cargo)@0.1.3

## 0.2.0 — 2026-04-14

### Minor changes

- [893ef59](https://github.com/bruits/satteri/commit/893ef59125e5969f34650ee27c919f1fae29fe62) Fix MDX import/export and expression handling to match the behavior of the original JavaScript implementation:
  
  - Fix `mdxjsEsm` nodes not being delivered to HAST plugin visitors
  - Fix multiline `export` blocks (e.g. objects, arrays) being truncated
  - Fix expression boundaries for edge cases involving comments, template literals, regex, and JSX
  - Report errors for unclosed MDX expressions — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri-ast (Cargo)@0.1.2

