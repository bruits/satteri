import { defineConfig } from "vite";
import satteri from "vite-plugin-satteri";

export default defineConfig({
  plugins: [
    satteri({
      // Toggle .md handling. Default: true.
      markdown: true,

      // Toggle .mdx handling. Pass `true` for defaults, `false` to disable,
      // or an object to configure the MDX compile.
      mdx: {
        jsxImportSource: "preact",
      },

      // Shared options applied to both .md and .mdx
      features: {
        gfm: true,
        frontmatter: true,
      },
    }),
  ],
  resolve: {
    alias: {
      react: "preact/compat",
      "react-dom": "preact/compat",
    },
  },
});
