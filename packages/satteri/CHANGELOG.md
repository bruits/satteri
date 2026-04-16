# satteri

## 0.2.3 — 2026-04-16

### Patch changes

- [ae83450](https://github.com/bruits/satteri/commit/ae83450e535f965d45be64aa83bc12806acb827b) Fixed optimizeStatic silently collapsing elements that have runtime component overrides via `export const components` — Thanks @Princesseuh!

## 0.2.2 — 2026-04-15

### Patch changes

- [6f08f69](https://github.com/bruits/satteri/commit/6f08f69b3304ac12b643e6f582faa3c01859b400) Fixes missing `optionalDependencies` field. — Thanks @Princesseuh!

## 0.2.1 — 2026-04-15

### Patch changes

- [b0cdb9b](https://github.com/bruits/satteri/commit/b0cdb9b8a01eaff8fb4aa6d02cdeee080241bcfb) Added `parseExpression()` to `mdxjsEsm` nodes, allowing ESM import/export statements to be parsed into ESTree ASTs. — Thanks @Princesseuh!

## 0.2.0 — 2026-04-14

### Minor changes

- [893ef59](https://github.com/bruits/satteri/commit/893ef59125e5969f34650ee27c919f1fae29fe62) Fix MDX import/export and expression handling to match the behavior of the original JavaScript implementation:
  - Fix `mdxjsEsm` nodes not being delivered to HAST plugin visitors
  - Fix multiline `export` blocks (e.g. objects, arrays) being truncated
  - Fix expression boundaries for edge cases involving comments, template literals, regex, and JSX
  - Report errors for unclosed MDX expressions — Thanks @Princesseuh!

### Patch changes

- [ecaeb2c](https://github.com/bruits/satteri/commit/ecaeb2ce18cbe6a7dc46d19bc49a32aa7114a2c5) Fixes browser export still bringing in Node code by accident. — Thanks @Princesseuh!
- [ecaeb2c](https://github.com/bruits/satteri/commit/ecaeb2ce18cbe6a7dc46d19bc49a32aa7114a2c5) Add position data to hast nodes. Position information was already stored in the Rust arena during mdast-to-hast conversion, but was never exposed to the JavaScript side. — Thanks @Princesseuh!
