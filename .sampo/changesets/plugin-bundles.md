---
npm/satteri: minor
npm/vite-plugin-satteri: minor
---

Added support for nested arrays in `mdastPlugins` and `hastPlugins`, so a package can export a bundle of plugins that you pass without spreading it. A bundle's plugins run in their own order, at the bundle's position. A factory can return a bundle as well as a single plugin, giving its plugins state they share with each other and reset per document.

```js
import { typography } from "some-package"; // an array of plugins

markdownToHtml(source, { mdastPlugins: [typography, myPlugin] });
```
