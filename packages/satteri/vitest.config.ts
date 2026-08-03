import codspeedPlugin from "@codspeed/vitest-plugin";
import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [codspeedPlugin()],
  test: {
    environment: "node",
    include: ["test/**/*.test.ts"],
    // The fuzz properties are a local discovery tool: they seed from the clock,
    // so in CI they turn an unrelated red build into an unreproducible one.
    // Anything they find gets pinned in fuzz-regressions.test.ts, which stays.
    exclude: [
      ...configDefaults.exclude,
      ...(process.env.SKIP_FUZZ ? ["test/conformance/fuzz/**"] : []),
    ],
    benchmark: {
      include: ["bench/**/*.bench.ts"],
    },
  },
});
