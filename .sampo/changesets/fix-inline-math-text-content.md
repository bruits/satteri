---
cargo/satteri-ast: patch
npm/satteri: patch
---

Fixes `ctx.textContent()` not including inline math. A heading like `# Energy $E=mc^2$` would only return `Energy ` instead of `Energy E=mc^2`.
