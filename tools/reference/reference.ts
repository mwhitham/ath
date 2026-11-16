/**
 * Locations shared by the reference generators in this folder.
 *
 * The generators run the 2.1.0 program, which this repository no longer
 * holds. ATH_REFERENCE_ROOT names a separate checkout of the reference commit
 * with its dependencies installed (see README.md). The old source, its
 * packages, and the synthetic inputs come from that checkout. The generated
 * files are written into this repository.
 */
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const REFERENCE_COMMIT = "c6218e88c035708bdd15b3a20888d19713e1df29";

/** This repository, where generated files are written. */
export const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** The checkout of the reference commit. */
export const reference = referenceRoot();

function referenceRoot(): string {
  const value = process.env.ATH_REFERENCE_ROOT;
  if (value === undefined || value === "") {
    throw new Error(
      `Set ATH_REFERENCE_ROOT to a checkout of ${REFERENCE_COMMIT} with its dependencies installed. ` +
        "See tools/reference/README.md.",
    );
  }
  const root = resolve(value);
  if (!existsSync(join(root, "src", "program.ts")) || !existsSync(join(root, "node_modules"))) {
    throw new Error(`${root} is not a checkout of the 2.1.0 reference with its dependencies installed.`);
  }
  return root;
}

/** Import a source file of the reference checkout, such as `src/db/index.ts`. */
export async function referenceModule<T = any>(path: string): Promise<T> {
  return (await import(pathToFileURL(join(reference, path)).href)) as T;
}

/**
 * Import a package installed in the reference checkout, such as `zod`, as an
 * ES module would. The old source imports the same file, so both share one
 * module instance.
 */
export async function referencePackage<T = any>(name: string): Promise<T> {
  const directory = realpathSync(join(reference, "node_modules", name));
  const manifest = JSON.parse(readFileSync(join(directory, "package.json"), "utf8")) as {
    exports?: unknown;
    module?: string;
    main?: string;
  };
  const exported = manifest.exports === undefined ? undefined : pick(rootExport(manifest.exports));
  const entry = exported ?? manifest.module ?? manifest.main ?? "index.js";
  return (await import(pathToFileURL(join(directory, entry)).href)) as T;
}

function rootExport(exports: unknown): unknown {
  if (typeof exports === "object" && exports !== null && "." in exports) {
    return (exports as Record<string, unknown>)["."];
  }
  return exports;
}

/** Node's condition matching for an ES module import. */
function pick(target: unknown): string | undefined {
  if (typeof target === "string") return target;
  if (Array.isArray(target)) {
    for (const item of target) {
      const found = pick(item);
      if (found !== undefined) return found;
    }
    return undefined;
  }
  if (typeof target === "object" && target !== null) {
    for (const [condition, value] of Object.entries(target)) {
      if (["node", "import", "default"].includes(condition)) {
        const found = pick(value);
        if (found !== undefined) return found;
      }
    }
  }
  return undefined;
}
