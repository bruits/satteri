# satteri-ast

## 0.1.2 — 2026-04-14

### Patch changes

- [893ef59](https://github.com/bruits/satteri/commit/893ef59125e5969f34650ee27c919f1fae29fe62) Fix MDX import/export and expression handling to match the behavior of the original JavaScript implementation:
  
  - Fix `mdxjsEsm` nodes not being delivered to HAST plugin visitors
  - Fix multiline `export` blocks (e.g. objects, arrays) being truncated
  - Fix expression boundaries for edge cases involving comments, template literals, regex, and JSX
  - Report errors for unclosed MDX expressions — Thanks @Princesseuh!
- [ecaeb2c](https://github.com/bruits/satteri/commit/ecaeb2ce18cbe6a7dc46d19bc49a32aa7114a2c5) Add position data to hast nodes. Position information was already stored in the Rust arena during mdast-to-hast conversion, but was never exposed to the JavaScript side. — Thanks @Princesseuh!

