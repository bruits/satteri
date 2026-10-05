---
npm/satteri: patch
---

Added Cloudflare Workers support with the separately installed `@bruits/satteri-wasm32-wasip1` binding. Use normal `satteri` imports without Node compatibility flags or manual initialization; Node-only installs do not download the WASM binding.
