/**
 * ath — the Athletic Standard command tree.
 *
 * A prediction needs a model. In a harness the model is already there. In a
 * bare terminal, predict and backtest call a gateway the person opted into.
 *
 * `cli.ts` parses this. `scripts/generate-skill-cli.ts` reads its help, so the
 * skill's command reference is built from the same source as `--help` (D80).
 */
import { Command } from "commander";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline/promises";
import {
  ATHLETIC_STANDARD_VERSION,
  ErasureReasonCode,
  PredictionData,
  ProviderReportingPeriodType,
  RecordType,
  SERIES_QUANTITY_UNITS as V1_SERIES_QUANTITY_UNITS,
  SeriesQuantitySchema,
} from "./v1/schema.js";
import { findFile, DEFAULT_FILENAME, FILE_SUFFIX } from "./file.js";
import { SEED_BENCHMARKS } from "./benchmarks.js";
import { PredictRefusal } from "./predict-errors.js";
import {
  clearKey,
  describeKeyStatus,
  envGateways,
  isGatewayName,
  KeyRefusal,
  savedGateways,
  setKey,
} from "./keyring.js";
import { loadCatalog, renderModelList, requireOnList, resolveModel, writeDefaultModel } from "./models.js";
import { complete } from "./gateway.js";
import { latestReport, shareRefusal } from "./share.js";
import { bareGuide, EXAMPLES, GROUPS, HELP_FOOTER } from "./help.js";
import {
  ALWAYS_DIR,
  installSkill,
  shippedSkillVersion,
  skillSource,
  skillStatus,
  staleSkillWarning,
  UNVERSIONED,
  type SkillCopy,
} from "./skill.js";
import { candidatePaths, runUpdate } from "./update.js";
import {
  checkDatabase,
  CALCULATION_OPERATIONS,
  CALCULATION_RECORD_KINDS,
  calculate,
  addAttachmentsToLogPlan,
  assertPackageConflictSetCurrent,
  clearPackageConflictSet,
  conflictResolutions,
  correctRecord,
  createDatabase,
  databaseStats,
  deleteRecord,
  eraseRecord,
  exportRecords,
  gradePrediction,
  importProviderRecords,
  importRecords,
  listRecords,
  loadAthleticStandardPackage,
  openDatabase,
  planLogEntry,
  planPredictionGrade,
  prepareAttachments,
  previewPackageImport,
  previewProviderImport,
  previewRecordErasure,
  readPackageConflictSet,
  readImportedPackageRecord,
  readRecord,
  readRecordLinks,
  readRecordRevisions,
  readSeriesView,
  renderDatabaseStats,
  renderCalculation,
  renderRecord,
  renderRecordList,
  renderSeriesView,
  runV1Backtest,
  writeV1BacktestReport,
  applyLogPlan,
  buildEvidence,
  resolveAsOfTime,
  resolvePackageConflict,
  savePackageConflictSet,
  writeRecord,
  writeRecordChanges,
  writeRecords,
  withPreparedAttachmentFiles,
  type AthleticDatabase,
  type LogPlan,
  type PackageImportResolution,
} from "./db/index.js";

/** The whole command tree. `cli.ts` parses it; the skill generator reads its help. */
export function buildProgram(): Command {
  const program = new Command();

  program
    .name("ath")
    // --update belongs to ath itself. Without this, Commander treats a program
    // option as valid on every command, so `ath log --update` would be ignored (D81).
    .enablePositionalOptions()
    .description(
      "Athletic Standard — an open, local-first format for training and recovery data " +
        "that keeps measured signals apart from self-reported ones.",
    )
    .version(ATHLETIC_STANDARD_VERSION)
    .option(
      "--update",
      "install the latest ath with the tool that installed it, and replace the agent skill if one is already in this folder",
    )
    .addHelpText("after", HELP_FOOTER)
    .action(async (opts: { update?: boolean }) => {
      if (opts.update) {
        const code = await runUpdate({
          packagePaths: candidatePaths(process.argv[1] ?? ""),
          currentVersion: ATHLETIC_STANDARD_VERSION,
          cwd: process.cwd(),
        });
        if (code !== 0) process.exit(code);
        return;
      }
      console.log(bareGuide(process.cwd()));
    });

  // One stderr line when an installed skill copy is older than the shipped one
  // (D79). Skipped for init and skill, which write it, and for --json runs, whose
  // reader is an agent that cannot act on it. Silent when no copy exists.
  program.hook("preAction", (_thisCommand, actionCommand) => {
    const chain: string[] = [];
    for (let c: Command | null = actionCommand; c; c = c.parent) chain.unshift(c.name());
    const top = chain[1];
    if (top === undefined || top === "init" || top === "skill") return;
    if (actionCommand.opts().json === true || process.argv.includes("--json")) return;
    const warning = staleSkillWarning(process.cwd());
    if (warning) console.error(warning);
  });

  program
    .command("init")
    .helpGroup(GROUPS.setUp)
    .description(`create a new ${DEFAULT_FILENAME} here, with the well-known benchmarks defined`)
    .addHelpText("after", EXAMPLES.init!)
    .option("--name <name>", "your name, so the file says whose it is")
    .option("--birth-year <year>", "birth year, which some readings are read against")
    .option("--sex <sex>", "male | female, for the same reason")
    .option("--units <units>", "metric | imperial — display only; stored values are always metric", "metric")
    .option("--timezone <name>", "default IANA timezone for calendar views, such as America/Chicago")
    .option("--file <path>", "write somewhere other than the default name", DEFAULT_FILENAME)
    .option("-y, --yes", "skip the questions and use the flags and defaults")
    .option("--empty", "create no athlete records, for restoring a v1 package")
    .option("--no-skill", "do not install the agent skill, even if an agent folder is here")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (opts) => {
      const outPath = resolve(process.cwd(), opts.file);
      if (!outPath.endsWith(FILE_SUFFIX)) {
        fail(`the live database name must end in ${FILE_SUFFIX}: ${outPath}`);
      }
      if (existsSync(outPath)) {
        fail(
          `${outPath} already exists, and overwriting it would lose whatever is in it. ` +
            `Read it with \`ath stats\`, or start a second file with \`--file <name>${FILE_SUFFIX}\`.`,
        );
      }

      let { name, birthYear, sex, timezone } = {
        name: opts.name,
        birthYear: opts.birthYear,
        sex: opts.sex,
        timezone: opts.timezone,
      };
      if (!opts.yes && !opts.json) {
        const rl = createInterface({ input: process.stdin, output: process.stdout });
        name = name ?? ((await rl.question("Name (optional): ")) || undefined);
        birthYear = birthYear ?? ((await rl.question("Birth year (optional): ")) || undefined);
        sex = sex ?? ((await rl.question("Sex, male/female (optional): ")) || undefined);
        timezone =
          timezone ??
          ((await rl.question("Default timezone, such as America/Chicago (optional): ")) ||
            undefined);
        rl.close();
      }

      const createdAt = new Date().toISOString();
      const toolActor = {
        author_type: "tool" as const,
        tool: "ath",
        tool_version: ATHLETIC_STANDARD_VERSION,
      };
      let database: AthleticDatabase | undefined;
      let seeded;
      try {
        database = await createDatabase(outPath, { createdAt });
        if (!opts.empty) seeded = await writeRecords(database, [
          {
            record_type: "athlete_profile",
            created_at: createdAt,
            changed_by: toolActor,
            data: {
              ...(name ? { name } : {}),
              ...(birthYear ? { birth_year: Number(birthYear) } : {}),
              ...(sex === "male" || sex === "female" ? { sex } : {}),
              display_units: opts.units === "imperial" ? "imperial" : "metric",
              ...(timezone ? { default_time_zone: timezone } : {}),
            },
          },
          {
            record_type: "source",
            created_at: createdAt,
            changed_by: toolActor,
            data: {
              source_type: "manual",
              data_writer: "ath",
              data_route: "manual_entry",
              detail: "Manually recorded data",
            },
          },
          ...SEED_BENCHMARKS.map((benchmark) => ({
            record_type: "benchmark_definition" as const,
            created_at: createdAt,
            changed_by: toolActor,
            data: {
              authorship: {
                author: {
                  ...toolActor,
                  calculation_version: "benchmark-definition-v1",
                },
                authored_at: createdAt,
                evidence: [],
                content_version: 1,
                status: "active" as const,
              },
              benchmark_key: benchmark.id,
              benchmark_type: benchmark.kind,
              score_type: benchmark.score_type,
              definition: benchmark.definition,
              tags: benchmark.tags,
            },
          })),
        ]);
      } catch (error) {
        database?.close();
        await Promise.all(
          [outPath, `${outPath}-wal`, `${outPath}-shm`].map((path) =>
            rm(path, { force: true }),
          ),
        );
        return fail(
          `could not create a valid v1 database: ${(error as Error).message}. Nothing was kept.`,
        );
      }
      database.close();

      // The skill always goes to .agents/skills, which every major harness reads,
      // and into any harness folder already here (D79).
      const installed = opts.skill === false ? [] : installSkill(process.cwd());

      if (opts.json) {
        console.log(
          JSON.stringify(
            {
              created: outPath,
              athleticstandard_version: ATHLETIC_STANDARD_VERSION,
              database_revision: seeded?.database_revision ?? 0,
              benchmark_record_ids: (seeded?.records ?? [])
                .filter((record) => record.record_type === "benchmark_definition")
                .map((record) => record.record_id),
              skill_installed: installed,
            },
            null,
            2,
          ),
        );
        return;
      }
      console.log(`created ${outPath}`);
      if (opts.empty) {
        console.log("  empty database, ready for a v1 package restore");
      } else {
        console.log(`  seeded ${SEED_BENCHMARKS.length} benchmarks: ${SEED_BENCHMARKS.map((b) => b.id).join(", ")}`);
      }
      for (const path of installed) console.log(`  installed the agent skill into ${path}`);
      if (installed.length > 0) {
        console.log(
          `  ${ALWAYS_DIR}/skills is read by Claude Code, Cursor, Codex, and Gemini CLI. ` +
            `\`ath skill install\` refreshes every copy after an upgrade.`,
        );
      } else if (opts.skill !== false) {
        console.log(`  the skill was not installed. \`ath skill install\` does it later.`);
      }
      console.log(`  next: \`ath import <export-file>\` to load device data`);
    });

  const skillCmd = program
    .command("skill")
    .helpGroup(GROUPS.setUp)
    .description("where the agent skill is installed, and whether each copy is current")
    .addHelpText("after", EXAMPLES.skill!)
    .option("--json", "structured output, for an agent rather than a person")
    .action((opts: { json?: boolean }) => {
      const copies = skillStatus(process.cwd());
      if (opts.json) {
        console.log(JSON.stringify({ shipped: shippedSkillVersion(), copies }, null, 2));
        return;
      }
      console.log(renderSkillStatus(process.cwd(), copies));
    });

  skillCmd
    .command("install")
    .description(`copy the skill into ${ALWAYS_DIR}/skills and every harness folder here`)
    .option("--json", "structured output, for an agent rather than a person")
    .action((opts: { json?: boolean }) => {
      const written = installSkill(process.cwd());
      // Commander hands `--json` to the parent when both declare it.
      if (opts.json || skillCmd.opts().json) {
        console.log(JSON.stringify({ shipped: shippedSkillVersion(), skill_installed: written }, null, 2));
        return;
      }
      if (written.length === 0) return fail(`the skill is not here to copy: expected it at ${skillSource()}`);
      for (const path of written) console.log(`installed the agent skill into ${path}`);
      console.log(`version ${shippedSkillVersion()}`);
    });

  program
    .command("import")
    .helpGroup(GROUPS.getIn)
    .description("preview and import a v1 package or Apple Health, WHOOP, or Oura export")
    .addHelpText("after", EXAMPLES.import!)
    .argument("<path>", "v1 package or supported provider export")
    .option("--file <path>", "import into a database other than the one in this folder")
    .option("-y, --yes", "apply a conflict-free preview")
    .option("--json", "structured preview; never confirms the write")
    .action(async (
      importPath: string,
      opts: { file?: string; yes?: boolean; json?: boolean },
    ) => {
      const athletePath = findOrFail(opts.file);
      const database = await openOrFail(athletePath, opts.json);
      try {
        const resolvedImportPath = resolve(process.cwd(), importPath);
        if (!isAthleticStandardPackagePath(resolvedImportPath)) {
          const preview = await previewProviderImport(database, resolvedImportPath);
          const output = {
            provider: preview.provider,
            imported: preview.label,
            destination_database_revision: preview.database_revision,
            new_records: preview.new_records,
            matching_records: preview.matching_records,
            changed_records: preview.changed_records,
            conflicts: preview.conflicts,
            skipped: preview.skipped,
            write_confirmed: false,
          };
          if (opts.json) {
            console.log(JSON.stringify(output, null, 2));
            return;
          }
          console.log(renderProviderImportPreview(output));
          if (!opts.yes) {
            console.log("nothing written; pass --yes after reviewing this preview");
            return;
          }
          if (preview.conflicts.length > 0) {
            return fail(
              `import has ${preview.conflicts.length} unresolved conflict` +
                `${preview.conflicts.length === 1 ? "" : "s"}; nothing was written`,
            );
          }
          const imported = await importProviderRecords(database, preview);
          console.log(
            `imported ${preview.label}: ${imported.new_record_count} new, ` +
              `${imported.changed_record_count} changed, ` +
              `${imported.matching_record_count} matching at database revision ` +
              `${imported.database_revision}`,
          );
          return;
        }
        const loaded = await loadAthleticStandardPackage(resolvedImportPath);
        const preview = await previewPackageImport(database, loaded);
        const output = {
          source_database_id: preview.source_database_id,
          source_database_revision: preview.source_database_revision,
          destination_database_id: preview.destination_database_id,
          destination_database_revision: preview.destination_database_revision,
          restore: preview.restore,
          new_records: preview.new_records,
          new_record_count: preview.new_record_count,
          matching_records: preview.matching_records,
          matching_record_count: preview.matching_record_count,
          changed_records: preview.changed_records,
          changed_record_count: preview.changed_record_count,
          record_lists_truncated: preview.record_lists_truncated,
          conflicts: preview.conflicts,
          unrepresentable_records: preview.unrepresentable_records,
          unknown_fields: preview.unknown_fields,
          write_confirmed: false,
        };
        if (opts.json) {
          if (preview.conflicts.length > 0) {
            await savePackageConflictSet(database, preview);
          } else {
            await clearPackageConflictSet(database);
          }
          console.log(JSON.stringify(output, null, 2));
          return;
        }
        console.log(renderImportPreview(output));
        if (!opts.yes) {
          if (preview.conflicts.length > 0) {
            const saved = await savePackageConflictSet(database, preview);
            if (saved !== null) {
              console.log(
                "review with ath conflicts show, then record each choice with ath conflicts resolve",
              );
            }
          } else {
            await clearPackageConflictSet(database);
          }
          console.log("nothing written; pass --yes after reviewing this preview");
          return;
        }
        let resolutions: PackageImportResolution[] = [];
        if (preview.conflicts.length > 0) {
          const state = await readPackageConflictSet(database);
          await assertPackageConflictSetCurrent(database, state, preview);
          resolutions = conflictResolutions(state);
          if (resolutions.length !== preview.conflicts.length) {
            return fail(
              `import has ${preview.conflicts.length - resolutions.length} unresolved ` +
                `conflict${preview.conflicts.length - resolutions.length === 1 ? "" : "s"}; ` +
                "use ath conflicts show and ath conflicts resolve",
            );
          }
        }
        const imported = await importRecords(database, preview, resolutions);
        await clearPackageConflictSet(database);
        console.log(
          `${imported.restored ? "restored" : "imported"} package at database revision ` +
            `${imported.database_revision}`,
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  const conflictsCmd = program
    .command("conflicts")
    .helpGroup(GROUPS.getIn)
    .description("review and resolve the temporary conflicts from a package import")
    .addHelpText("after", EXAMPLES.conflicts!);

  conflictsCmd
    .command("show")
    .description("show complete local and imported values from the current conflict preview")
    .argument("[record-id]", "show one conflicting stable record ID")
    .option("--file <path>", "read conflicts for a database other than the one in this folder")
    .option("--json", "structured output")
    .action(async (
      recordId: string | undefined,
      opts: { file?: string; json?: boolean },
    ) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const state = await readPackageConflictSet(database);
        const loaded = await loadAthleticStandardPackage(state.source_path);
        const preview = await previewPackageImport(database, loaded);
        await assertPackageConflictSetCurrent(database, state, preview);
        const selected =
          recordId === undefined
            ? state.conflicts
            : state.conflicts.filter(
                (conflict) => conflict.record_id === recordId,
              );
        if (selected.length === 0) {
          return fail(`record is not in the temporary conflict set: ${recordId}`);
        }
        const conflicts = await Promise.all(
          selected.map(async (conflict) => ({
            record_id: conflict.record_id,
            message: conflict.message,
            local: await readRecord(database, conflict.record_id),
            imported: await readImportedPackageRecord(
              loaded,
              conflict.record_id,
            ),
            resolution: conflict.resolution ?? null,
          })),
        );
        const output = {
          source_path: state.source_path,
          source_database_revision: state.source_database_revision,
          destination_database_revision: state.destination_database_revision,
          conflicts,
        };
        console.log(JSON.stringify(output, null, 2));
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  conflictsCmd
    .command("resolve")
    .description("choose the local, imported, or a corrected value for one conflict")
    .argument("<record-id>", "conflicting stable record ID")
    .requiredOption("--choice <choice>", "local, imported, or corrected")
    .option("--data <json>", "complete corrected data object; required with corrected")
    .option("--file <path>", "resolve conflicts for a database other than the one in this folder")
    .option("--json", "structured output")
    .action(async (
      recordId: string,
      opts: {
        choice: string;
        data?: string;
        file?: string;
        json?: boolean;
      },
    ) => {
      if (!["local", "imported", "corrected"].includes(opts.choice)) {
        return fail("--choice must be local, imported, or corrected");
      }
      if (opts.choice === "corrected" && opts.data === undefined) {
        return fail("--data is required when --choice is corrected");
      }
      if (opts.choice !== "corrected" && opts.data !== undefined) {
        return fail("--data is used only when --choice is corrected");
      }
      let data: unknown;
      if (opts.data !== undefined) {
        try {
          data = JSON.parse(opts.data);
        } catch (error) {
          return fail(`--data is not valid JSON: ${(error as Error).message}`);
        }
      }
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const state = await readPackageConflictSet(database);
        const loaded = await loadAthleticStandardPackage(state.source_path);
        const preview = await previewPackageImport(database, loaded);
        await assertPackageConflictSetCurrent(database, state, preview);
        const resolution: PackageImportResolution =
          opts.choice === "corrected"
            ? { record_id: recordId, choice: "corrected", data }
            : {
                record_id: recordId,
                choice: opts.choice as "local" | "imported",
              };
        const next = await resolvePackageConflict(
          database,
          state,
          recordId,
          resolution,
        );
        const unresolved = next.conflicts.filter(
          (conflict) => conflict.resolution === undefined,
        ).length;
        const output = {
          record_id: recordId,
          choice: resolution.choice,
          unresolved_conflicts: unresolved,
          athlete_data_changed: false,
        };
        if (opts.json) {
          console.log(JSON.stringify(output, null, 2));
        } else {
          console.log(
            `recorded ${resolution.choice} for ${recordId}; ${unresolved} unresolved ` +
              `conflict${unresolved === 1 ? "" : "s"}`,
          );
          console.log(
            `nothing written to athlete data; run ath import ${state.source_path} --yes after all conflicts are resolved`,
          );
        }
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("export")
    .helpGroup(GROUPS.getIn)
    .description("write a complete deterministic Athletic Standard JSON package")
    .addHelpText("after", EXAMPLES.export!)
    .argument("<directory>", "new directory to create for the package")
    .option("--exported-at <timestamp>", "include this exact UTC export time in the manifest")
    .option("--file <path>", "export a database other than the one in this folder")
    .option("--json", "structured command result")
    .action(async (
      directory: string,
      opts: { exportedAt?: string; file?: string; json?: boolean },
    ) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const result = await exportRecords(
          database,
          resolve(process.cwd(), directory),
          opts.exportedAt === undefined
            ? {}
            : { exported_at: opts.exportedAt },
        );
        if (opts.json) {
          console.log(JSON.stringify(result, null, 2));
          return;
        }
        console.log(
          `exported database revision ${result.database_revision} to ${result.directory}`,
        );
        console.log("  athlete.ath.json");
        console.log("  export-manifest.json");
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("log")
    .helpGroup(GROUPS.getIn)
    .description("write one plain-text record or one validated structured batch")
    .addHelpText("after", EXAMPLES.log!)
    .argument("[entry...]", "the entry, unquoted. Leave it off to paste one, ending with Ctrl-D")
    .option("--benchmark <name>", "name the workout yourself, instead of naming it after the day")
    .option("--at <timestamp>", "exact UTC event time with milliseconds; defaults to now")
    .option("--agent <name>", "name the agent making a structured write")
    .option(
      "--attachment <path>",
      "attach a PNG, JPEG, WebP, or PDF; pass more than once for several files",
      collectValues,
      [] as string[],
    )
    .option("--file <path>", "write to a file other than the one in this folder")
    .option("-y, --yes", "write it without asking — for scripts, and when you are sure")
    .option("--dry-run", "show what would be written, and write nothing")
    .option("--json", "structured preview for an agent; never confirms the write")
    .action(async (words: string[], opts: V1LogOptions) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const text = words.length > 0 ? words.join(" ") : await readStdin();
        const actor =
          opts.agent === undefined
            ? ({ author_type: "athlete" } as const)
            : ({ author_type: "agent", agent: opts.agent } as const);
        let plan = await planLogEntry(database, {
          text,
          actor,
          ...(opts.at === undefined ? {} : { created_at: opts.at }),
          ...(opts.benchmark === undefined ? {} : { benchmark: opts.benchmark }),
        });
        const attachments = await prepareAttachments(
          database,
          opts.attachment ?? [],
          actor,
          plan.inputs[0]?.created_at ?? new Date().toISOString(),
        );
        plan = addAttachmentsToLogPlan(plan, attachments);
        const preview = {
          file: path,
          interpretation: plan.interpretation,
          records: plan.inputs.map((input) => ({
            record_id: input.record_id,
            record_type: input.record_type,
            created_at: input.created_at,
            data: input.data,
          })),
          write_confirmed: false,
        };
        if (opts.json) {
          console.log(JSON.stringify(preview, null, 2));
          return;
        }
        console.log(renderLogPlan(plan));
        if (opts.dryRun) {
          console.log("nothing written (--dry-run)");
          return;
        }
        if (!(await confirmWrite(opts, "Write these records?"))) {
          console.log("nothing written");
          return;
        }
        const written = await withPreparedAttachmentFiles(
          database,
          attachments,
          () => applyLogPlan(database, plan),
        );
        console.log(
          `wrote ${written.records.length} record${written.records.length === 1 ? "" : "s"} ` +
            `at database revision ${written.database_revision}`,
        );
        for (const record of written.records) {
          console.log(`  ${record.record_id}  ${record.record_type}`);
        }
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("link")
    .helpGroup(GROUPS.getIn)
    .description("attach a benchmark result to the workout session it happened in")
    .addHelpText("after", EXAMPLES.link!)
    .argument("<result-id>", "stable benchmark-result record ID")
    .argument("<session-id>", "stable workout-session record ID")
    .requiredOption("--revision <number>", "current result revision from the preview", parsePositiveInteger)
    .option("--file <path>", "change a database other than the one in this folder")
    .option("-y, --yes", "write without asking after reviewing the preview")
    .option("--dry-run", "show what would change and write nothing")
    .option("--json", "structured preview; never confirms the write")
    .action(async (
      resultId: string,
      sessionId: string,
      opts: { revision: number; file?: string; yes?: boolean; dryRun?: boolean; json?: boolean },
    ) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const result = await readRecord(database, resultId);
        if (result?.record_type !== "benchmark_result" || result.is_deleted) {
          return fail(`${resultId} is not a current benchmark result`);
        }
        const session = await readRecord(database, sessionId);
        if (session?.record_type !== "workout_session" || session.is_deleted) {
          return fail(`${sessionId} is not a current workout session`);
        }
        const preview = {
          file: path,
          record_id: result.record_id,
          expected_revision: opts.revision,
          current_revision: result.current_revision,
          old_workout_session_id: result.data.workout_session_id ?? null,
          new_workout_session_id: session.record_id,
          write_confirmed: false,
        };
        if (opts.json) {
          console.log(JSON.stringify(preview, null, 2));
          return;
        }
        console.log(
          `result ${result.record_id} revision ${result.current_revision}\n` +
            `  workout session: ${result.data.workout_session_id ?? "none"} → ${session.record_id}`,
        );
        if (opts.dryRun) {
          console.log("nothing written (--dry-run)");
          return;
        }
        if (!(await confirmWrite(opts, "Write this link as a correction?"))) {
          console.log("nothing written");
          return;
        }
        const corrected = await correctRecord(database, {
          record_id: result.record_id,
          expected_revision: opts.revision,
          changed_by: { author_type: "athlete" },
          data: { ...result.data, workout_session_id: session.record_id },
        });
        console.log(
          `linked ${result.record_id} to ${session.record_id} at revision ${corrected.record.current_revision}`,
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("correct")
    .helpGroup(GROUPS.getIn)
    .description("replace one record's complete data and retain its prior revision")
    .addHelpText("after", EXAMPLES.correct!)
    .argument("<record-id>", "stable record ID")
    .requiredOption("--revision <number>", "current revision from the preview", parsePositiveInteger)
    .requiredOption("--data <json>", "complete replacement for the record's data object")
    .option("--agent <name>", "name the agent making the correction")
    .option(
      "--attachment <path>",
      "attach a PNG, JPEG, WebP, or PDF; pass more than once for several files",
      collectValues,
      [] as string[],
    )
    .option("--file <path>", "change a database other than the one in this folder")
    .option("-y, --yes", "write without asking after reviewing the preview")
    .option("--dry-run", "show what would change and write nothing")
    .option("--json", "structured preview; never confirms the write")
    .action(async (
      recordId: string,
      opts: {
        revision: number;
        data: string;
        agent?: string;
        attachment?: string[];
        file?: string;
        yes?: boolean;
        dryRun?: boolean;
        json?: boolean;
      },
    ) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const current = await readRecord(database, recordId);
        if (current === null || current.is_deleted) {
          return fail(`current record not found: ${recordId}`);
        }
        let data: unknown;
        try {
          data = JSON.parse(opts.data);
        } catch (error) {
          return fail(`--data is not valid JSON: ${(error as Error).message}`);
        }
        const actor =
          opts.agent === undefined
            ? ({ author_type: "athlete" } as const)
            : ({ author_type: "agent", agent: opts.agent } as const);
        const attachments = await prepareAttachments(
          database,
          opts.attachment ?? [],
          actor,
          new Date().toISOString(),
        );
        if (
          attachments.length > 0 &&
          current.record_type !== "athlete_report" &&
          current.record_type !== "nutrition_entry"
        ) {
          return fail(
            "--attachment can correct only an athlete_report or nutrition_entry",
          );
        }
        if (
          attachments.length > 0 &&
          (typeof data !== "object" || data === null || Array.isArray(data))
        ) {
          return fail("--data must be a complete record data object");
        }
        const correctedData =
          attachments.length === 0
            ? data
            : {
                ...(data as Record<string, unknown>),
                attachment_ids: [
                  ...(
                    (data as { attachment_ids?: string[] }).attachment_ids ??
                    []
                  ),
                  ...attachments.map(
                    (attachment) => attachment.input.record_id!,
                  ),
                ],
              };
        const preview = {
          file: path,
          record_id: current.record_id,
          record_type: current.record_type,
          expected_revision: opts.revision,
          current_revision: current.current_revision,
          before: current.data,
          after: correctedData,
          attachments: attachments.map((attachment) => ({
            record_id: attachment.input.record_id,
            data: attachment.input.data,
          })),
          write_confirmed: false,
        };
        if (opts.json) {
          console.log(JSON.stringify(preview, null, 2));
          return;
        }
        console.log(JSON.stringify(preview, null, 2));
        if (opts.dryRun) {
          console.log("nothing written (--dry-run)");
          return;
        }
        if (!(await confirmWrite(opts, "Write this correction?"))) {
          console.log("nothing written");
          return;
        }
        const corrected =
          attachments.length === 0
            ? await correctRecord(database, {
                record_id: current.record_id,
                expected_revision: opts.revision,
                changed_by: actor,
                data: correctedData,
              })
            : await withPreparedAttachmentFiles(
                database,
                attachments,
                async () => {
                  const written = await writeRecordChanges(database, {
                    records: attachments.map(
                      (attachment) => attachment.input,
                    ),
                    corrections: [{
                      record_id: current.record_id,
                      expected_revision: opts.revision,
                      changed_by: actor,
                      data: correctedData,
                    }],
                  });
                  const record = written.records.find(
                    (record) => record.record_id === current.record_id,
                  );
                  if (record === undefined) {
                    throw new Error("attachment correction did not return its record");
                  }
                  return {
                    record,
                    database_revision: written.database_revision,
                  };
                },
              );
        console.log(
          `corrected ${current.record_id} to revision ${corrected.record.current_revision} ` +
            `(database revision ${corrected.database_revision})`,
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("delete")
    .helpGroup(GROUPS.getIn)
    .description("hide one current record while retaining its complete history")
    .addHelpText("after", EXAMPLES.delete!)
    .argument("<record-id>", "stable record ID")
    .requiredOption("--revision <number>", "current revision from the preview", parsePositiveInteger)
    .option("--file <path>", "change a database other than the one in this folder")
    .option("-y, --yes", "write without asking after reviewing the preview")
    .option("--dry-run", "show what would change and write nothing")
    .option("--json", "structured preview; never confirms the write")
    .action(async (
      recordId: string,
      opts: {
        revision: number;
        file?: string;
        yes?: boolean;
        dryRun?: boolean;
        json?: boolean;
      },
    ) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const current = await readRecord(database, recordId);
        if (current === null || current.is_deleted) {
          return fail(`current record not found: ${recordId}`);
        }
        const preview = {
          file: path,
          record_id: current.record_id,
          record_type: current.record_type,
          expected_revision: opts.revision,
          current_revision: current.current_revision,
          retained_history: true,
          write_confirmed: false,
        };
        if (opts.json) {
          console.log(JSON.stringify(preview, null, 2));
          return;
        }
        console.log(JSON.stringify(preview, null, 2));
        if (opts.dryRun) {
          console.log("nothing written (--dry-run)");
          return;
        }
        if (!(await confirmWrite(opts, "Delete this record from current views?"))) {
          console.log("nothing written");
          return;
        }
        const deleted = await deleteRecord(database, {
          record_id: current.record_id,
          expected_revision: opts.revision,
          deleted_by: { author_type: "athlete" },
        });
        console.log(
          `deleted ${deleted.record_id} at database revision ${deleted.database_revision}; history retained`,
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("erase")
    .helpGroup(GROUPS.getIn)
    .description("permanently remove one record and every record that depends on it")
    .addHelpText("after", EXAMPLES.erase!)
    .argument("<record-id>", "stable record ID to erase")
    .requiredOption("--revision <number>", "current revision from the preview", parsePositiveInteger)
    .option(
      "--reason <code>",
      `one of: ${ErasureReasonCode.options.join(", ")}`,
      "athlete_request",
    )
    .option("--confirm-erasure <record-id>", "second confirmation: repeat the record ID exactly")
    .option("--file <path>", "change a database other than the one in this folder")
    .option("-y, --yes", "accept the first confirmation after reviewing the preview")
    .option("--dry-run", "show everything that would be erased and write nothing")
    .option("--json", "structured preview; never confirms the erasure")
    .action(async (
      recordId: string,
      opts: {
        revision: number;
        reason: string;
        confirmErasure?: string;
        file?: string;
        yes?: boolean;
        dryRun?: boolean;
        json?: boolean;
      },
    ) => {
      const reason = ErasureReasonCode.safeParse(opts.reason);
      if (!reason.success) {
        return fail(
          `unknown erasure reason '${opts.reason}'. Use ${ErasureReasonCode.options.join(", ")}`,
        );
      }
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const preview = await previewRecordErasure(database, recordId, opts.revision);
        const output = {
          file: path,
          root_record_id: preview.root_record_id,
          expected_revision: preview.root_expected_revision,
          database_revision: preview.database_revision,
          permanent: true,
          records: preview.records,
          attachments: preview.attachments,
          reason_code: reason.data,
          write_confirmed: false,
        };
        if (opts.json) {
          console.log(JSON.stringify(output, null, 2));
          return;
        }
        console.log(JSON.stringify(output, null, 2));
        console.log(
          `WARNING: this permanently removes ${preview.records.length} record` +
            `${preview.records.length === 1 ? "" : "s"} and cannot be undone from the database.`,
        );
        if (opts.dryRun) {
          console.log("nothing erased (--dry-run)");
          return;
        }
        if (!(await confirmWrite(opts, "Permanently erase these records?"))) {
          console.log("nothing erased");
          return;
        }
        let secondConfirmation = opts.confirmErasure;
        if (secondConfirmation === undefined) {
          if (!process.stdin.isTTY || !process.stdout.isTTY) {
            return fail(
              "second confirmation is required in non-interactive use; pass " +
                `--confirm-erasure ${preview.root_record_id}`,
            );
          }
          secondConfirmation = await askOnTerminal(
            `Type the record ID ${preview.root_record_id} to confirm permanent erasure: `,
          ) ?? undefined;
        }
        if (secondConfirmation !== preview.root_record_id) {
          return fail(
            "second confirmation did not exactly match the record ID; nothing was erased",
          );
        }
        const erased = await eraseRecord(database, {
          preview,
          reason_code: reason.data,
        });
        console.log(
          `erased ${erased.erased_record_ids.length} record` +
            `${erased.erased_record_ids.length === 1 ? "" : "s"} and ` +
            `${erased.erased_attachment_count} attachment file` +
            `${erased.erased_attachment_count === 1 ? "" : "s"} ` +
            `at database revision ${erased.database_revision}`,
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("check")
    .helpGroup(GROUPS.read)
    .description("make sure the file still obeys every rule of the format")
    .addHelpText("after", EXAMPLES.check!)
    .argument("[file]", "the file to check (default: the one in this folder)")
    .option("--file <path>", "check a database other than the one in this folder")
    .option("--validate-export", "also build and validate a temporary complete JSON package")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (
      fileArg,
      opts: { file?: string; validateExport?: boolean; json?: boolean },
    ) => {
      const path = findOrFail(opts.file ?? fileArg);
      const database = await openOrFail(path, opts.json);
      try {
        const result = await checkDatabase(database);
        let exportValidated = false;
        if (opts.validateExport) {
          const temporary = await mkdtemp(join(tmpdir(), "ath-check-export-"));
          try {
            await exportRecords(database, join(temporary, "package"));
            exportValidated = true;
          } finally {
            await rm(temporary, { recursive: true, force: true });
          }
        }
        if (opts.json) {
          console.log(
            JSON.stringify(
              { file: path, ...result, export_validated: exportValidated },
              null,
              2,
            ),
          );
        } else {
          for (const issue of result.issues) {
            const target = issue.record_id === undefined ? "" : ` ${issue.record_id}`;
            const line = `${issue.severity}  ${issue.check}${target}: ${issue.message}`;
            if (issue.severity === "error") console.error(line);
            else console.warn(line);
          }
          if (result.valid) {
            console.log(
              `${path}: valid Athletic Standard ${result.metadata?.athleticstandard_version ?? ATHLETIC_STANDARD_VERSION} database ` +
                `(${result.record_count} records)`,
            );
            if (exportValidated) {
              console.log("ok  export: complete JSON package validated");
            }
          } else {
            console.error(
              `\n${path}: INVALID — ${result.issues.filter((issue) => issue.severity === "error").length} error(s)`,
            );
          }
        }
        if (!result.valid) {
          process.exitCode = 1;
        }
      } finally {
        database.close();
      }
    });

  const keyCmd = program
    .command("key")
    .helpGroup(GROUPS.setUp)
    .description("save or show a gateway key — never stored in the athlete file")
    .addHelpText("after", EXAMPLES.key!)
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (opts: { json?: boolean }) => {
      try {
        const saved = await savedGateways();
        const fromEnv = envGateways();
        if (opts.json) {
          console.log(JSON.stringify({ saved, environment: fromEnv }, null, 2));
          return;
        }
        console.log(describeKeyStatus(saved, fromEnv));
      } catch (e) {
        if (e instanceof KeyRefusal) return fail((e as Error).message);
        throw e;
      }
    });

  keyCmd
    .command("set")
    .description("save a Vercel or OpenRouter key in the OS password store")
    .argument("<gateway>", "vercel or openrouter")
    .action(async (gatewayArg: string) => {
      const gateway = parseGateway(gatewayArg);
      try {
        await setKey(gateway);
        console.log(`saved a ${gateway} key in the password store. The key is not printed.`);
      } catch (e) {
        if (e instanceof KeyRefusal) return fail((e as Error).message);
        throw e;
      }
    });

  keyCmd
    .command("clear")
    .description("delete the saved gateway key from the password store")
    .argument("[gateway]", "vercel or openrouter; leave off to clear both")
    .action(async (gatewayArg?: string) => {
      const gateway = gatewayArg === undefined ? undefined : parseGateway(gatewayArg);
      try {
        const cleared = await clearKey(gateway);
        if (cleared.length === 0) console.log("no gateway key was saved.");
        else console.log(`cleared ${cleared.join(" and ")} from the password store.`);
      } catch (e) {
        if (e instanceof KeyRefusal) return fail((e as Error).message);
        throw e;
      }
    });

  program
    .command("models")
    .helpGroup(GROUPS.setUp)
    .description("list live text models that can reason, or save your usual one")
    .addHelpText("after", EXAMPLES.models!)
    .option("--default <name>", "save this model next to the athlete file, for the next predict")
    .option("--gateway <name>", "vercel or openrouter, when both keys are available")
    .option("--file <path>", "the athlete file whose default to save")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (opts: { default?: string; gateway?: string; file?: string; json?: boolean }) => {
      try {
        const gateway = opts.gateway ? parseGateway(opts.gateway) : undefined;
        const { gateway: used, models } = await loadCatalog(gateway);
        if (opts.default) {
          const path = findOrFail(opts.file);
          const id = requireOnList(opts.default, models);
          writeDefaultModel(path, id);
          if (opts.json) {
            console.log(JSON.stringify({ default: id, file: path, gateway: used }, null, 2));
            return;
          }
          console.log(`saved ${id} as the default model next to the athlete file.`);
          return;
        }
        if (opts.json) {
          console.log(JSON.stringify({ gateway: used, models }, null, 2));
          return;
        }
        console.log(renderModelList(models, used));
      } catch (e) {
        if (e instanceof PredictRefusal || e instanceof KeyRefusal) return fail((e as Error).message);
        throw e;
      }
    });

  program
    .command("predict")
    .helpGroup(GROUPS.predict)
    .description("predict a benchmark — needs a model in a bare terminal; --json is evidence")
    .addHelpText("after", EXAMPLES.predict!)
    .argument("<benchmark>", "the benchmark to predict, e.g. `fran`. See them all with `ath stats`")
    .option("--model <name>", "the model for this run. See them with `ath models`")
    .option("--gateway <name>", "vercel or openrouter, when both keys are available")
    .option("--as-of <time>", "historical day (YYYY-MM-DD) or exact UTC timestamp")
    .option("--timezone <name>", "IANA timezone when --as-of is a calendar day")
    .option("--file <path>", "read a file other than the one in this folder")
    .option("-y, --yes", "write the prediction without asking")
    .option("--dry-run", "show the prediction, and write nothing")
    .option("--json", "evidence for a harness, not a prediction")
    .action(async (benchmarkId: string, opts: PredictCliOptions) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const asOf = await resolveAsOfTime(database, opts.asOf, opts.timezone);
        const evidence = await buildEvidence(database, {
          benchmark: benchmarkId,
          as_of: asOf.as_of,
        });
        if (opts.json) {
          console.log(
            JSON.stringify(
              {
                ...evidence,
                ...(asOf.time_zone === undefined
                  ? {}
                  : { calendar_time_zone: asOf.time_zone }),
              },
              null,
              2,
            ),
          );
          return;
        }
        const selected = await resolveModel(
          path,
          opts.model,
          opts.gateway === undefined ? undefined : parseGateway(opts.gateway),
        );
        const response = await complete({
          gateway: selected.gateway,
          model: selected.model,
          system:
            "Return only JSON with predicted_score, optional predicted_range, confidence, and reasoning. " +
            "Use the benchmark score fields duration_s, reps, or weight_kg. Do not add medical advice.",
          user: JSON.stringify(evidence),
        });
        const modelOutput = parsePredictionResponse(response.content);
        const authoredAt = new Date().toISOString();
        const evidenceReferences = [
          evidence.benchmark,
          ...evidence.prior_results,
          ...evidence.device_measurements,
          ...evidence.athlete_reports,
          ...evidence.sleep_sessions,
          ...evidence.workout_sessions,
        ].map((record) => ({
          record_id: record.record_id,
          revision: record.current_revision,
        }));
        const data = PredictionData.parse({
          authorship: {
            author: {
              author_type: "agent",
              agent: "ath-terminal",
              agent_version: ATHLETIC_STANDARD_VERSION,
              model: response.model,
            },
            authored_at: authoredAt,
            evidence: evidenceReferences,
            content_version: 1,
            status: "active",
          },
          benchmark_definition_id: evidence.benchmark.record_id,
          ...modelOutput,
        });
        const preview = {
          benchmark: evidence.benchmark.data.benchmark_key,
          model: response.model,
          data,
          write_confirmed: false,
        };
        console.log(JSON.stringify(preview, null, 2));
        if (opts.dryRun) {
          console.log("nothing written (--dry-run)");
          return;
        }
        if (!(await confirmWrite(opts, "Save this prediction?"))) {
          console.log("nothing written");
          return;
        }
        const written = await writeRecord(database, {
          record_type: "prediction",
          created_at: authoredAt,
          changed_by: {
            author_type: "agent",
            agent: "ath-terminal",
            agent_version: ATHLETIC_STANDARD_VERSION,
          },
          data,
        });
        console.log(
          `saved prediction ${written.record.record_id} at database revision ${written.database_revision}`,
        );
      } catch (error) {
        if (error instanceof PredictRefusal || error instanceof KeyRefusal) {
          return fail(error.message);
        }
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("backtest")
    .helpGroup(GROUPS.predict)
    .description("replay history with one or more models — writes a report, not the athlete file")
    .addHelpText("after", EXAMPLES.backtest!)
    .option("-m, --model <name>", "a model to run; pass more than once to compare", collectModels, [] as string[])
    .option("--all", "run every live text model that can reason, after showing the count")
    .option("--gateway <name>", "vercel or openrouter, when both keys are available")
    .option("--file <path>", "read a file other than the one in this folder")
    .option("-y, --yes", "do not ask before a long --all run")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (opts: BacktestCliOptions) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const requestedGateway =
          opts.gateway === undefined ? undefined : parseGateway(opts.gateway);
        let gateway: "vercel" | "openrouter";
        let models: string[];
        if (opts.all) {
          const catalog = await loadCatalog(requestedGateway);
          gateway = catalog.gateway;
          models = catalog.models.map((model) => model.id);
        } else if (opts.model !== undefined && opts.model.length > 0) {
          const catalog = await loadCatalog(requestedGateway);
          gateway = catalog.gateway;
          models = opts.model.map((model) => requireOnList(model, catalog.models));
        } else {
          const selected = await resolveModel(path, undefined, requestedGateway);
          gateway = selected.gateway;
          models = [selected.model];
        }
        if (opts.all && !opts.yes) {
          if (!(await confirmWrite(opts, `Run a backtest with ${models.length} models?`))) {
            console.log("nothing run");
            return;
          }
        }
        const beforeRevision = (await databaseStats(database)).database_revision;
        const report = await runV1Backtest(database, {
          models,
          gateway,
          complete,
        });
        const afterRevision = (await databaseStats(database)).database_revision;
        if (beforeRevision !== afterRevision) {
          return fail("backtest changed the athlete database; report was not written");
        }
        const reportPath = await writeV1BacktestReport(path, report);
        if (opts.json) {
          console.log(JSON.stringify({ report: reportPath, ...report }, null, 2));
          return;
        }
        console.log(
          `replayed ${report.replayable_result_count} results with ${models.length} model` +
            `${models.length === 1 ? "" : "s"}`,
        );
        for (const summary of report.summary) {
          console.log(
            `  ${summary.model}: ${summary.completed} completed, ${summary.failed} failed, ` +
              `median absolute error ${summary.median_absolute_error_percentage ?? "—"}%`,
          );
        }
        console.log(`wrote ${reportPath}`);
      } catch (error) {
        if (error instanceof PredictRefusal || error instanceof KeyRefusal) {
          return fail(error.message);
        }
        if (error instanceof Error && error.message === "nothing run") {
          console.log("nothing run");
          return;
        }
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("share")
    .helpGroup(GROUPS.predict)
    .description("not built yet — names the latest backtest report")
    .addHelpText("after", EXAMPLES.share!)
    .option("--file <path>", "look next to a file other than the one in this folder")
    .option("--json", "structured output, for an agent rather than a person")
    .action((opts: { file?: string; json?: boolean }) => {
      let path: string | undefined;
      try {
        path = findFile(opts.file);
      } catch {
        path = undefined;
      }
      const report = path
        ? latestReport(path)
        : latestReport(join(process.cwd(), DEFAULT_FILENAME));
      if (opts.json) {
        console.log(JSON.stringify({ built: false, latest_report: report ?? null }, null, 2));
        return;
      }
      console.log(shareRefusal(report));
    });

  program
    .command("grade")
    .helpGroup(GROUPS.predict)
    .description("grade an existing prediction against an existing benchmark result")
    .addHelpText("after", EXAMPLES.grade!)
    .argument("<prediction-id>", "stable prediction record ID")
    .requiredOption("--result <record-id>", "stable benchmark-result record ID")
    .option("--calculation-version <version>", "grade calculation version", "prediction-grade-v1")
    .option("--file <path>", "change a database other than the one in this folder")
    .option("-y, --yes", "write without asking after reviewing the preview")
    .option("--dry-run", "show what would be written, and write nothing")
    .option("--json", "structured preview; never confirms the write")
    .action(async (
      predictionId: string,
      opts: {
        result: string;
        calculationVersion: string;
        file?: string;
        yes?: boolean;
        dryRun?: boolean;
        json?: boolean;
      },
    ) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const plan = await planPredictionGrade(database, {
          prediction_id: predictionId,
          result_id: opts.result,
          calculation_version: opts.calculationVersion,
          actor: {
            author_type: "tool",
            tool: "ath",
            tool_version: ATHLETIC_STANDARD_VERSION,
          },
        });
        const preview = {
          file: path,
          prediction: {
            record_id: plan.prediction.record_id,
            revision: plan.prediction.current_revision,
          },
          result: {
            record_id: plan.result.record_id,
            revision: plan.result.current_revision,
          },
          prediction_result_link:
            plan.link_input !== null
              ? {
                  action: "create",
                  record_id: plan.link_input.record_id,
                  data: plan.link_input.data,
                }
              : plan.link_correction !== null
                ? {
                    action: "revise",
                    record_id: plan.existing_link!.record_id,
                    current_revision: plan.existing_link!.current_revision,
                    new_revision: plan.existing_link!.current_revision + 1,
                    data: plan.link_correction.data,
                  }
              : {
                  action: "reuse",
                  record_id: plan.existing_link!.record_id,
                  revision: plan.existing_link!.current_revision,
                },
          grade: {
            record_id: plan.grade_input.record_id,
            data: plan.grade_input.data,
          },
          write_confirmed: false,
        };
        if (opts.json) {
          console.log(JSON.stringify(preview, null, 2));
          return;
        }
        console.log(JSON.stringify(preview, null, 2));
        if (opts.dryRun) {
          console.log("nothing written (--dry-run)");
          return;
        }
        if (!(await confirmWrite(opts, "Write this grade?"))) {
          console.log("nothing written");
          return;
        }
        const written = await gradePrediction(database, plan);
        if (written.grade.record_type !== "prediction_grade") {
          return fail("grade write returned the wrong record type");
        }
        console.log(
          `${written.grade.data.result_in_predicted_range ? "hit" : "miss"} — signed error ` +
            `${written.grade.data.signed_error}, absolute error ` +
            `${written.grade.data.absolute_error_percentage}%`,
        );
        console.log(
          `grade ${written.grade.record_id} at database revision ${written.database_revision}`,
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("calculate")
    .helpGroup(GROUPS.read)
    .description("run a named calculation without writing athlete data")
    .addHelpText("after", EXAMPLES.calculate!)
    .argument("<operation>", `one of: ${CALCULATION_OPERATIONS.join(", ")}`)
    .argument("<quantity>", "measurement quantity, provider metric, or summary type")
    .option(
      "--record-kind <kind>",
      `one of: ${CALCULATION_RECORD_KINDS.join(", ")}`,
      "measurement",
    )
    .option(
      "--period-type <type>",
      "provider period type; required for provider-period-summary",
    )
    .option("--source <id>", "use one source only")
    .option("--compare-source <id>", "second source for compare-sources or correlation")
    .option("--other-quantity <quantity>", "second quantity for correlation")
    .option("--session <record-id>", "workout session for workout-drift")
    .option("--from <time>", "earliest day (YYYY-MM-DD) or exact UTC timestamp")
    .option("--to <time>", "latest day (YYYY-MM-DD) or exact UTC timestamp")
    .option("--timezone <name>", "IANA timezone for calendar operations")
    .option("--group <period>", "none, day, week, or month", "none")
    .option("--file <path>", "read a database other than the one in this folder")
    .option("--json", "structured calculation receipt")
    .action(async (
      operation: string,
      quantity: string,
      opts: {
        recordKind: string;
        periodType?: string;
        source?: string;
        compareSource?: string;
        otherQuantity?: string;
        session?: string;
        from?: string;
        to?: string;
        timezone?: string;
        group: string;
        file?: string;
        json?: boolean;
      },
    ) => {
      if (!CALCULATION_OPERATIONS.includes(operation as never)) {
        return fail(
          `unknown calculation '${operation}'. Use ${CALCULATION_OPERATIONS.join(", ")}`,
        );
      }
      if (!["none", "day", "week", "month"].includes(opts.group)) {
        return fail("unknown --group value. Use none, day, week, or month");
      }
      if (!CALCULATION_RECORD_KINDS.includes(opts.recordKind as never)) {
        return fail(
          `unknown --record-kind '${opts.recordKind}'. Use ` +
            CALCULATION_RECORD_KINDS.join(", "),
        );
      }
      const periodType =
        opts.periodType === undefined
          ? undefined
          : ProviderReportingPeriodType.safeParse(opts.periodType);
      if (periodType !== undefined && !periodType.success) {
        return fail(
          `unknown --period-type '${opts.periodType}'. Use ` +
            ProviderReportingPeriodType.options.join(", "),
        );
      }
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const calculation = await calculate(database, {
          operation: operation as (typeof CALCULATION_OPERATIONS)[number],
          quantity,
          record_kind:
            opts.recordKind as (typeof CALCULATION_RECORD_KINDS)[number],
          ...(periodType === undefined
            ? {}
            : { provider_period_type: periodType.data }),
          group: opts.group as "none" | "day" | "week" | "month",
          ...(opts.source === undefined ? {} : { source_id: opts.source }),
          ...(opts.compareSource === undefined
            ? {}
            : { compare_source_id: opts.compareSource }),
          ...(opts.otherQuantity === undefined
            ? {}
            : { other_quantity: opts.otherQuantity }),
          ...(opts.session === undefined
            ? {}
            : { workout_session_id: opts.session }),
          ...(opts.from === undefined ? {} : { from: opts.from }),
          ...(opts.to === undefined ? {} : { to: opts.to }),
          ...(opts.timezone === undefined ? {} : { time_zone: opts.timezone }),
        });
        console.log(
          opts.json
            ? JSON.stringify(calculation, null, 2)
            : renderCalculation(calculation),
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("series")
    .helpGroup(GROUPS.read)
    .description("read a sample series back: one row per day, or the raw samples")
    .addHelpText("after", EXAMPLES.series!)
    .argument("<quantity>", `one of: ${Object.keys(V1_SERIES_QUANTITY_UNITS).join(", ")}`)
    .option("--from <time>", "earliest day (YYYY-MM-DD) or exact UTC timestamp")
    .option("--to <time>", "latest day (YYYY-MM-DD) or exact UTC timestamp")
    .option("--source <id>", "one source only, for when two measured the same thing — `ath stats` lists the ids")
    .option("--timezone <name>", "IANA timezone for calendar days; uses the athlete profile when omitted")
    .option("--limit <count>", "maximum samples to read", parsePositiveInteger, 10_000)
    .option("--raw", "every sample, rather than one row per day")
    .option("--json", "structured output, for an agent rather than a person")
    .option("--file <path>", "read a file other than the one in this folder")
    .action(async (
      quantity: string,
      opts: {
        from?: string;
        to?: string;
        source?: string;
        timezone?: string;
        limit: number;
        raw?: boolean;
        json?: boolean;
        file?: string;
      },
    ) => {
      const parsedQuantity = SeriesQuantitySchema.safeParse(quantity);
      if (!parsedQuantity.success) {
        return fail(
          `unknown quantity '${quantity}'. Known quantities: ` +
            Object.keys(V1_SERIES_QUANTITY_UNITS).join(", ") +
            `. \`ath stats\` lists the ones this database actually holds.`,
        );
      }
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const view = await readSeriesView(database, parsedQuantity.data, {
          ...(opts.from === undefined ? {} : { from: opts.from }),
          ...(opts.to === undefined ? {} : { to: opts.to }),
          ...(opts.source === undefined ? {} : { source_id: opts.source }),
          ...(opts.timezone === undefined ? {} : { time_zone: opts.timezone }),
          ...(opts.raw ? { raw: true } : {}),
          limit: opts.limit,
        });
        if ((view.samples?.length ?? view.days?.length ?? 0) === 0) {
          return fail(
            opts.source
              ? `no ${quantity} samples from '${opts.source}'. Drop --source or run \`ath stats\` for source IDs.`
              : `no ${quantity} samples matched. Run \`ath stats\` to see the recorded series.`,
          );
        }
        console.log(opts.json ? JSON.stringify(view, null, 2) : renderSeriesView(view));
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  program
    .command("stats")
    .helpGroup(GROUPS.read)
    .description("what is in the file: counts, date ranges, sources, and baselines")
    .addHelpText("after", EXAMPLES.stats!)
    .argument("[file]", "the file to summarize (default: the one in this folder)")
    .option("--file <path>", "summarize a database other than the one in this folder")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (fileArg, opts: { file?: string; json?: boolean }) => {
      const path = findOrFail(opts.file ?? fileArg);
      const database = await openOrFail(path, opts.json);
      try {
        const stats = await databaseStats(database);
        console.log(
          opts.json ? JSON.stringify(stats, null, 2) : renderDatabaseStats(stats),
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  const recordsCommand = program
    .command("records")
    .helpGroup(GROUPS.read)
    .description("find stable record IDs, inspect one record, and read its revision history")
    .addHelpText("after", EXAMPLES.records!);

  recordsCommand
    .command("list")
    .description("list current records in stable order")
    .option("--type <record-type>", "show one record type")
    .option("--include-deleted", "include records hidden by a normal deletion")
    .option("--limit <count>", "maximum records to return", parsePositiveInteger, 100)
    .option("--file <path>", "read a database other than the one in this folder")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (opts: {
      type?: string;
      includeDeleted?: boolean;
      limit: number;
      file?: string;
      json?: boolean;
    }) => {
      const recordType = opts.type === undefined ? undefined : RecordType.safeParse(opts.type);
      if (recordType !== undefined && !recordType.success) {
        return fail(
          `unknown record type '${opts.type}'. Known types: ${RecordType.options.join(", ")}`,
        );
      }
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const records = await listRecords(database, {
          ...(recordType === undefined ? {} : { record_type: recordType.data }),
          ...(opts.includeDeleted ? { include_deleted: true } : {}),
          limit: opts.limit,
        });
        console.log(
          opts.json
            ? JSON.stringify({ file: path, records }, null, 2)
            : renderRecordList(records),
        );
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  recordsCommand
    .command("show")
    .description("show one record, one revision, its history, or its links")
    .argument("<record-id>", "stable UUIDv7 record ID")
    .option("--revision <number>", "show this retained revision", parsePositiveInteger)
    .option("--history", "include the revision timeline")
    .option("--links", "include incoming and outgoing record links")
    .option("--file <path>", "read a database other than the one in this folder")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (
      recordId: string,
      opts: {
        revision?: number;
        history?: boolean;
        links?: boolean;
        file?: string;
        json?: boolean;
      },
    ) => {
      const path = findOrFail(opts.file);
      const database = await openOrFail(path, opts.json);
      try {
        const record = await readRecord(database, recordId);
        if (record === null) {
          return fail(`record not found: ${recordId}`);
        }
        const revisions =
          opts.revision !== undefined || opts.history
            ? await readRecordRevisions(database, recordId)
            : undefined;
        const shownRevision =
          opts.revision === undefined
            ? undefined
            : revisions?.find((revision) => revision.revision === opts.revision);
        if (opts.revision !== undefined && shownRevision === undefined) {
          return fail(
            `record ${recordId} has no revision ${opts.revision}; current revision is ${record.current_revision}`,
          );
        }
        const links = opts.links ? await readRecordLinks(database, recordId) : undefined;
        if (opts.json) {
          console.log(
            JSON.stringify(
              {
                file: path,
                record,
                ...(shownRevision === undefined
                  ? {}
                  : { shown_revision: shownRevision }),
                ...(opts.history ? { history: revisions } : {}),
                ...(links === undefined ? {} : { links }),
              },
              null,
              2,
            ),
          );
        } else {
          console.log(
            renderRecord(record, {
              ...(shownRevision === undefined ? {} : { revision: shownRevision }),
              ...(opts.history && revisions !== undefined ? { history: revisions } : {}),
              ...(links === undefined ? {} : { links }),
            }),
          );
        }
      } catch (error) {
        return fail((error as Error).message);
      } finally {
        database.close();
      }
    });

  return program;
}

interface V1LogOptions {
  benchmark?: string;
  at?: string;
  agent?: string;
  attachment?: string[];
  file?: string;
  yes?: boolean;
  dryRun?: boolean;
  json?: boolean;
}

interface PredictCliOptions {
  model?: string;
  gateway?: string;
  asOf?: string;
  timezone?: string;
  file?: string;
  yes?: boolean;
  dryRun?: boolean;
  json?: boolean;
}

interface BacktestCliOptions {
  model?: string[];
  all?: boolean;
  gateway?: string;
  file?: string;
  yes?: boolean;
  json?: boolean;
}

function collectModels(value: string, prev: string[]): string[] {
  return [...prev, value];
}

function collectValues(value: string, previous: string[]): string[] {
  return [...previous, value];
}

function parsePositiveInteger(value: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`expected a positive whole number, not '${value}'`);
  }
  return parsed;
}

function parsePredictionResponse(content: string): Record<string, unknown> {
  const trimmed = content
    .trim()
    .replace(/^```(?:json)?\s*/i, "")
    .replace(/\s*```$/, "");
  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch (error) {
    throw new Error(`model did not return valid prediction JSON: ${(error as Error).message}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("model prediction response must be one JSON object");
  }
  return value as Record<string, unknown>;
}

function renderImportPreview(preview: {
  source_database_id: string;
  source_database_revision: number;
  destination_database_id: string;
  destination_database_revision: number;
  restore: boolean;
  new_records: readonly string[];
  new_record_count: number;
  matching_records: readonly string[];
  matching_record_count: number;
  changed_records: readonly string[];
  changed_record_count: number;
  record_lists_truncated: boolean;
  conflicts: readonly { record_id?: string; message: string }[];
  unrepresentable_records: readonly string[];
  unknown_fields: readonly string[];
}): string {
  const lines = [
    `${preview.restore ? "restore" : "merge"} preview`,
    `  source database:      ${preview.source_database_id} revision ${preview.source_database_revision}`,
    `  destination database: ${preview.destination_database_id} revision ${preview.destination_database_revision}`,
    `  new records:          ${preview.new_record_count}`,
    `  matching records:     ${preview.matching_record_count}`,
    `  changed records:      ${preview.changed_record_count}`,
    `  conflicts:            ${preview.conflicts.length}`,
    `  unrepresentable:       ${preview.unrepresentable_records.length}`,
    `  unknown fields:        ${preview.unknown_fields.length}`,
  ];
  if (preview.record_lists_truncated) {
    lines.push("  record ID lists:      first 1,000 per category");
  }
  for (const conflict of preview.conflicts) {
    lines.push(
      `    ${conflict.record_id === undefined ? "" : `${conflict.record_id}: `}` +
        conflict.message,
    );
  }
  return lines.join("\n");
}

function renderProviderImportPreview(preview: {
  imported: string;
  destination_database_revision: number;
  new_records: readonly { record_id: string; record_type: string }[];
  matching_records: readonly { record_id: string; record_type: string }[];
  changed_records: readonly {
    record_id: string;
    record_type: string;
    current_revision: number;
  }[];
  conflicts: readonly { record_id?: string; message: string }[];
  skipped: Readonly<Record<string, { count: number; example?: string }>>;
}): string {
  const skippedCount = Object.values(preview.skipped).reduce(
    (total, skipped) => total + skipped.count,
    0,
  );
  const lines = [
    `${preview.imported} preview`,
    `  destination revision: ${preview.destination_database_revision}`,
    `  new records:         ${preview.new_records.length}`,
    `  matching records:    ${preview.matching_records.length}`,
    `  changed records:     ${preview.changed_records.length}`,
    `  conflicts:           ${preview.conflicts.length}`,
    `  skipped rows:        ${skippedCount}`,
  ];
  for (const conflict of preview.conflicts) {
    lines.push(
      `    ${conflict.record_id === undefined ? "" : `${conflict.record_id}: `}` +
        conflict.message,
    );
  }
  for (const [reason, skipped] of Object.entries(preview.skipped)) {
    lines.push(
      `    skipped ${skipped.count} — ${reason}` +
        `${skipped.example === undefined ? "" : ` (example: ${skipped.example})`}`,
    );
  }
  return lines.join("\n");
}

function isAthleticStandardPackagePath(path: string): boolean {
  return (
    path.toLowerCase().endsWith(".json") ||
    existsSync(join(path, "athlete.ath.json"))
  );
}

function parseGateway(value: string): "vercel" | "openrouter" {
  if (isGatewayName(value)) return value;
  fail(`unknown gateway '${value}'. Use vercel or openrouter.`);
}

function renderLogPlan(plan: LogPlan): string {
  const lines = [
    `understood: ${plan.interpretation}`,
    `records to write: ${plan.inputs.length}`,
  ];
  for (const input of plan.inputs) {
    lines.push(`  ${input.record_id}  ${input.record_type}`);
    lines.push(
      ...JSON.stringify(input.data, null, 2)
        .split("\n")
        .map((line) => `    ${line}`),
    );
  }
  return lines.join("\n");
}

async function confirmWrite(
  opts: { yes?: boolean; dryRun?: boolean; json?: boolean },
  question: string,
): Promise<boolean> {
  if (opts.json || opts.dryRun) {
    return false;
  }
  if (opts.yes) {
    return true;
  }
  if (!process.stdout.isTTY) {
    fail(
      `there is no terminal to ask on, so nothing was written. Pass --yes after reviewing the preview.`,
    );
  }
  const answer = await askOnTerminal(`${question} [y] yes  [n] no `);
  return /^y(es)?$/i.test(answer ?? "");
}

/** Everything piped or pasted, to the end. Empty when nothing is coming. */
async function readStdin(): Promise<string> {
  if (process.stdin.isTTY) {
    process.stderr.write("Paste the entry, then press Ctrl-D:\n");
  }
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk));
  return Buffer.concat(chunks).toString("utf8");
}

/**
 * Ask on the terminal itself rather than on stdin.
 *
 * When the entry was pasted, stdin has already ended at Ctrl-D, so there is nothing
 * left to read an answer from. The terminal is still there, so the question goes to
 * it directly. Returns null where there is no terminal at all.
 */
async function askOnTerminal(question: string): Promise<string | null> {
  const { createReadStream } = await import("node:fs");
  let input: NodeJS.ReadableStream;
  try {
    input = process.stdin.isTTY && process.stdin.readable ? process.stdin : createReadStream("/dev/tty");
  } catch {
    return null;
  }
  const rl = createInterface({ input, output: process.stdout });
  try {
    return (await rl.question(question)).trim();
  } catch {
    return null;
  } finally {
    rl.close();
  }
}

function findOrFail(fileArg?: string): string {
  try {
    return findFile(fileArg);
  } catch (e) {
    return fail((e as Error).message);
  }
}

async function openOrFail(path: string, json = false): Promise<AthleticDatabase> {
  try {
    return await openDatabase(path, {
      onSecurityWarning: (warning) => {
        if (!json) console.warn(`ath: warning: ${warning}`);
      },
    });
  } catch (error) {
    return fail((error as Error).message);
  }
}

function fail(message: string): never {
  console.error(`ath: ${message}`);
  process.exit(1);
}

function renderSkillStatus(cwd: string, copies: SkillCopy[]): string {
  const lines = [`agent skill, version ${shippedSkillVersion()} shipped with this ath`, ``];
  for (const copy of copies) {
    const where = copy.path.slice(cwd.length).replace(/^[\\/]+/, "") || copy.path;
    const state =
      copy.state === "current"
        ? `current (${copy.installed})`
        : copy.state === "stale"
          ? `stale: ${copy.installed === UNVERSIONED ? "an older version" : copy.installed} installed`
          : "missing";
    lines.push(`  ${where.padEnd(40)} ${state}`);
  }
  lines.push(``);
  if (copies.some((c) => c.state !== "current")) {
    lines.push(`\`ath skill install\` writes the shipped version into each of these.`);
  } else {
    lines.push(`Every copy is current.`);
  }
  lines.push(`${ALWAYS_DIR}/skills is read by Claude Code, Cursor, Codex, and Gemini CLI.`);
  return lines.join("\n");
}
