/**
 * Generates schema/athleticstandard.schema.json from the Zod schemas.
 * The Zod definitions in src/schema.ts are the source of truth; run
 * `pnpm generate:json-schema` after any schema change and commit the result.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { AthleticStandardFile, ATHLETIC_STANDARD_VERSION } from "../src/schema.js";
import {
  ATHLETIC_STANDARD_VERSION as V1_ATHLETIC_STANDARD_VERSION,
  AthleticStandardPackage,
  ExportManifest,
  MeasurementSeriesFile,
} from "../src/v1/schema.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function generateSchema(
  schema: z.ZodType,
  id: string,
  title: string,
  outputPath: string,
): void {
  const jsonSchema = z.toJSONSchema(schema, {
    target: "draft-2020-12",
    io: "input",
    unrepresentable: "any",
  });

  const out = {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $id: id,
    title,
    ...jsonSchema,
  };

  const absolutePath = join(root, outputPath);
  mkdirSync(dirname(absolutePath), { recursive: true });
  writeFileSync(absolutePath, JSON.stringify(out, null, 2) + "\n");
  console.log(`wrote ${outputPath}`);
}

generateSchema(
  AthleticStandardFile,
  `https://athleticstandard.ai/schema/${ATHLETIC_STANDARD_VERSION}/athleticstandard.schema.json`,
  "Athletic Standard 0.4.3 file",
  `schema/${ATHLETIC_STANDARD_VERSION}/athleticstandard.schema.json`,
);

generateSchema(
  AthleticStandardPackage,
  `https://athleticstandard.ai/schema/${V1_ATHLETIC_STANDARD_VERSION}/athleticstandard.schema.json`,
  "Athletic Standard portable package document",
  `schema/${V1_ATHLETIC_STANDARD_VERSION}/athleticstandard.schema.json`,
);

generateSchema(
  AthleticStandardPackage,
  `https://athleticstandard.ai/schema/${V1_ATHLETIC_STANDARD_VERSION}/athleticstandard.schema.json`,
  "Athletic Standard portable package document",
  "schema/athleticstandard.schema.json",
);

generateSchema(
  ExportManifest,
  `https://athleticstandard.ai/schema/${V1_ATHLETIC_STANDARD_VERSION}/export-manifest.schema.json`,
  "Athletic Standard export manifest",
  `schema/${V1_ATHLETIC_STANDARD_VERSION}/export-manifest.schema.json`,
);

generateSchema(
  MeasurementSeriesFile,
  `https://athleticstandard.ai/schema/${V1_ATHLETIC_STANDARD_VERSION}/measurement-series.schema.json`,
  "Athletic Standard measurement-series file",
  `schema/${V1_ATHLETIC_STANDARD_VERSION}/measurement-series.schema.json`,
);
