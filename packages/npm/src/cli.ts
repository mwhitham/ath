#!/usr/bin/env node
/**
 * The `ath` command. It runs the packed native executable; see launcher.ts.
 */
import { fileURLToPath } from "node:url";
import { exitLike, main } from "./launcher.js";

exitLike(await main(process.argv.slice(2), process.argv[1], fileURLToPath(import.meta.url)));
