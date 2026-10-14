import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // CLI suites launch many tsx subprocesses. Running files in parallel lets
    // those subprocesses starve each other on shared CI runners and makes the
    // per-test timeout measure runner contention instead of command behavior.
    maxWorkers: 1,
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
