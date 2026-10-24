import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // CLI suites launch many tsx subprocesses. Running files in parallel lets
    // those subprocesses starve each other on shared CI runners and makes the
    // per-test timeout measure runner contention instead of command behavior.
    maxWorkers: 1,
    exclude: [...configDefaults.exclude],
  },
});
