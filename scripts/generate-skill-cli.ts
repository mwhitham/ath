/**
 * Writes skill/references/cli.md from the CLI's own help (D80).
 *
 * Run after changing a command or its help text. `pnpm test` fails when the
 * committed file is behind the code.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildProgram } from "../src/program.js";
import { renderCliReference } from "../src/skillcli.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "skill", "references", "cli.md");

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, renderCliReference(buildProgram()));
console.log(`wrote ${out}`);
