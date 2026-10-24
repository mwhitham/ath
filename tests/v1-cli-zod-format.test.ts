import { resolve } from "node:path";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ath } from "./helpers/cli-exec.js";

const demoFixture = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../examples/demo-athlete/athlete.ath.sqlite",
);

describe("CLI schema validation errors", () => {
  it("formats Zod errors as ath: path: message instead of raw JSON", () => {
    const result = ath(["records", "show", "not-a-valid-record-id", "--file", demoFixture], "/");
    expect(result.code).not.toBe(0);
    expect(result.stdout).toMatch(/^ath: /m);
    expect(result.stdout).not.toMatch(/^ath: \[/m);
    expect(result.stdout).toMatch(/record_id|uuid|Invalid/i);
  });
});
