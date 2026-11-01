import { realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { configDefaults, defineConfig } from "vitest/config";

// macOS keeps its temp folder behind the /var symlink, and the database code
// refuses a path through a symlink. Tests start from the real folder, as a
// person working in a real folder would.
const temporaryRoot = realpathSync(tmpdir());

export default defineConfig({
  test: {
    // CLI suites launch many tsx subprocesses. Running files in parallel lets
    // those subprocesses starve each other on shared CI runners and makes the
    // per-test timeout measure runner contention instead of command behavior.
    maxWorkers: 1,
    env: { TMPDIR: temporaryRoot, TMP: temporaryRoot, TEMP: temporaryRoot },
    exclude: [...configDefaults.exclude],
  },
});
