---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixed text inside an element nested in `<script>` or `<style>` rendering unescaped; only text directly inside those elements is left as-is.
