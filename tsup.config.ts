import { defineConfig } from "tsup";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    cli: "src/cli.ts",
    database: "src/db/index.ts",
    schema: "src/v1/schema.ts",
  },
  format: ["esm"],
  dts: true,
  clean: true,
  sourcemap: true,
});
