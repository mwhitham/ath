#!/usr/bin/env node
import { buildProgram } from "./program.js";
import { CliExitError } from "./cli/runtime.js";
import { releaseAllOpenLocalDatabases } from "./db/client.js";
import { formatSchemaValidationError } from "./db/validation-message.js";

async function main(): Promise<void> {
  try {
    await buildProgram().parseAsync(process.argv);
  } catch (error) {
    if (error instanceof CliExitError) {
      process.exitCode = error.exitCode;
    } else {
      console.error(`ath: ${formatSchemaValidationError(error)}`);
      process.exitCode = 1;
    }
  } finally {
    await releaseAllOpenLocalDatabases();
  }
}

void main();
