import { realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { defineConfig } from "vitest/config";

// macOS keeps its temporary folder behind the /var link, and the database
// refuses a path through a link. Tests start from the real folder.
const temporaryRoot = realpathSync(tmpdir());

export default defineConfig({
  test: {
    include: ["test/**/*.test.ts"],
    setupFiles: ["test/support.ts"],
    // Each client call starts one native worker. Running files one at a time
    // keeps runner contention out of the timing.
    maxWorkers: 1,
    testTimeout: 60_000,
    env: { TMPDIR: temporaryRoot, TMP: temporaryRoot, TEMP: temporaryRoot },
  },
});
