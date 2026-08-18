---
npm/satteri: patch
---

Added `visitMdastHook`, `visitHastHook` and `normalizePlugins` to the exports, so a hand-driven plugin pipeline can run `before`/`after` hooks and resolve plugin factories the way `markdownToHtml` does. The diagnostic and hook types (`MdastDiagnostic`, `HastDiagnostic`, `MdastHookFn`, `HastHookFn`) are exported alongside them.
