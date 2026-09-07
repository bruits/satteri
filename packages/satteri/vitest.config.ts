import codspeedPlugin from "@codspeed/vitest-plugin";
import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [codspeedPlugin()],
  test: {
    environment: "node",
    include: ["test/**/*.test.ts"],
    // Clock-seeded fuzz tests are not reproducible; CI runs their fixed regression cases instead.
    exclude: [
      ...configDefaults.exclude,
      ...(process.env.SKIP_FUZZ ? ["test/conformance/fuzz/**"] : []),
    ],
    benchmark: {
      include: ["bench/**/*.bench.ts"],
    },
  },
});
