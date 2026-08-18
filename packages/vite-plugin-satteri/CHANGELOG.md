# vite-plugin-satteri

## 0.3.0 — 2026-08-18

### Minor changes

- [e53e725](https://github.com/bruits/satteri/commit/e53e725e3eca758b5c65364b583c06a96d515510) Added a way to run a plugin only on some documents: a plugin factory now receives the file's `fileURL`, `sourceFormat`, `source` and `data`, and can return `null`, `undefined` or `false` to be left out for that document. Those skip values are also accepted anywhere a plugin entry can appear.
  
  ```js
  const onlyChangelogs = (ctx) =>
    ctx.fileURL?.pathname.endsWith("/CHANGELOG.md") ? rewriteVersions : null;
  
  markdownToHtml(source, { mdastPlugins: [onlyChangelogs, myPlugin] });
  ```
  
  Anything else in a plugin list now fails with an error naming the option and what it expected. — Thanks @Princesseuh!
- [8df3f76](https://github.com/bruits/satteri/commit/8df3f765b2df9cbfa1aa4130a126b9315e431c14) Added support for nested arrays in `mdastPlugins` and `hastPlugins`, so a package can export a bundle of plugins that you pass without spreading it. A bundle's plugins run in their own order, at the bundle's position. A factory can return a bundle as well as a single plugin, giving its plugins state they share with each other and reset per document.
  
  ```js
  import { typography } from "some-package"; // an array of plugins
  
  markdownToHtml(source, { mdastPlugins: [typography, myPlugin] });
  ```
   — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri (npm)@0.10.0

## 0.2.15 — 2026-07-08

### Patch changes

- Updated dependencies: satteri (npm)@0.9.5

## 0.2.14 — 2026-06-29

### Patch changes

- Updated dependencies: satteri (npm)@0.9.4

## 0.2.13 — 2026-06-25

### Patch changes

- Updated dependencies: satteri (npm)@0.9.3

## 0.2.12 — 2026-06-23

### Patch changes

- Updated dependencies: satteri (npm)@0.9.2

## 0.2.11 — 2026-06-19

### Patch changes

- Updated dependencies: satteri (npm)@0.9.1

## 0.2.10 — 2026-06-18

### Patch changes

- Updated dependencies: satteri (npm)@0.9.0

## 0.2.9 — 2026-06-11

### Patch changes

- Updated dependencies: satteri (npm)@0.8.2

## 0.2.8 — 2026-06-08

### Patch changes

- Updated dependencies: satteri (npm)@0.8.1

## 0.2.7 — 2026-06-03

### Patch changes

- Updated dependencies: satteri (npm)@0.8.0

## 0.2.6 — 2026-06-02

### Patch changes

- Updated dependencies: satteri (npm)@0.7.0

## 0.2.5 — 2026-05-21

### Patch changes

- Updated dependencies: satteri (npm)@0.6.3

## 0.2.4 — 2026-05-20

### Patch changes

- Updated dependencies: satteri (npm)@0.6.2

## 0.2.3 — 2026-05-19

### Patch changes

- Updated dependencies: satteri (npm)@0.6.1

## 0.2.2 — 2026-05-18

### Patch changes

- Updated dependencies: satteri (npm)@0.6.0

## 0.2.1 — 2026-05-12

### Patch changes

- Updated dependencies: satteri (npm)@0.5.1

## 0.2.0 — 2026-05-12

### Minor changes

- [57127a8](https://github.com/bruits/satteri/commit/57127a857f1692995b793fb155a64cb6b9ce0b21) Added TOML frontmatter parsing (+++ fenced blocks) alongside YAML — Thanks @Princesseuh!

### Patch changes

- Updated dependencies: satteri (npm)@0.5.0
