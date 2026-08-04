import codspeedPlugin from "@codspeed/vitest-plugin";
import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [codspeedPlugin()],
  test: {
    environment: "node",
    include: ["test/**/*.test.ts"],
    // Fuzz properties seed from the clock, so a CI failure isn't reproducible;
    // their finds are pinned in fuzz-regressions.test.ts instead.
    exclude: [
      ...configDefaults.exclude,
      ...(process.env.SKIP_FUZZ ? ["test/conformance/fuzz/**"] : []),
    ],
    benchmark: {
      include: ["bench/**/*.bench.ts"],
    },
  },
});
