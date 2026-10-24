import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const CLI = resolve(here, "../../src/cli.ts");
export const TSX = resolve(here, "../../node_modules/.bin/tsx");

export function ath(args: string[], cwd: string): { stdout: string; code: number } {
  try {
    const stdout = execFileSync(TSX, [CLI, ...args], { cwd, encoding: "utf8", stdio: "pipe" });
    return { stdout, code: 0 };
  } catch (error) {
    const err = error as { status: number; stdout?: string; stderr?: string };
    return { stdout: (err.stdout ?? "") + (err.stderr ?? ""), code: err.status };
  }
}
