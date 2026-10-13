import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    exclude: [
      ...configDefaults.exclude,
      // D101 removed the prerelease JSON runtime. These suites assert that retired
      // command contract. The v1 suites cover the SQLite commands.
      "tests/backtest.test.ts",
      "tests/docs.test.ts",
      "tests/grade.test.ts",
      "tests/import.test.ts",
      "tests/log.test.ts",
      "tests/predict.test.ts",
      "tests/seriesview.test.ts",
    ],
  },
});
