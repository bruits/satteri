import { defineConfig } from "vite";
import satteri from "vite-plugin-satteri";

export default defineConfig({
  plugins: [
    satteri({
      markdown: true,

      mdx: {
        jsxImportSource: "preact",
      },

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
