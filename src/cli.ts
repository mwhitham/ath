#!/usr/bin/env node
import { buildProgram } from "./program.js";
import { CliExitError } from "./cli/runtime.js";
import { releaseAllOpenLocalDatabases } from "./db/client.js";

async function main(): Promise<void> {
  try {
    await buildProgram().parseAsync(process.argv);
  } catch (error) {
    if (error instanceof CliExitError) {
      process.exitCode = error.exitCode;
    } else {
      const message = error instanceof Error ? error.message : String(error);
      console.error(`ath: ${message}`);
      process.exitCode = 1;
    }
  } finally {
    await releaseAllOpenLocalDatabases();
  }
}

void main();
