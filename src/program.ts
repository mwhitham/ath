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
import { rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline/promises";
import { type AthleticStandardFileT } from "./schema.js";
import {
  ATHLETIC_STANDARD_VERSION,
  RecordType,
  SERIES_QUANTITY_UNITS as V1_SERIES_QUANTITY_UNITS,
  SeriesQuantitySchema,
} from "./v1/schema.js";
import { validateAthleticStandardFile } from "./validate.js";
import { findFile, loadFile, saveFile, DEFAULT_FILENAME, FILE_SUFFIX } from "./file.js";
import { SEED_BENCHMARKS } from "./benchmarks.js";
import {
  applyDraft,
  buildDraft,
  draftAsJson,
  linkResult,
  LogRefusal,
  renderDraft,
  renderQuestion,
  renderWritten,
  today,
  waitingMatches,
  type Confirm,
} from "./log.js";
import { evidenceFor } from "./context.js";
import {
  benchmarkOrRefuse,
  evidenceAsJson,
  planPredict,
  PredictRefusal,
  renderPrediction,
} from "./predict.js";
import {
  clearKey,
  describeKeyStatus,
  envGateways,
  isGatewayName,
  KeyRefusal,
  savedGateways,
  setKey,
} from "./keyring.js";
import { loadCatalog, renderModelList, requireOnList, writeDefaultModel } from "./models.js";
import { planBacktest, renderReport, runBacktest, writeReport } from "./backtest.js";
import { latestReport, shareRefusal } from "./share.js";
import {
  analysisAsJson,
  applyGrade,
  attachAnalysis,
  gradeAsJson,
  GradeRefusal,
  planGrade,
  renderGrade,
} from "./grade.js";
import { bareGuide, EXAMPLES, GROUPS, HELP_FOOTER } from "./help.js";
import { importExport, PooledFileError, UnknownExportError } from "./import/index.js";
import { silentProgress, terminalProgress } from "./progress.js";
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
import { mergeSummaryAsJson, renderMergeSummary } from "./import/merge.js";
import { candidatePaths, runUpdate } from "./update.js";
import {
  checkDatabase,
  createDatabase,
  databaseStats,
  listRecords,
  openDatabase,
  readRecord,
  readRecordLinks,
  readRecordRevisions,
  readSeriesView,
  renderDatabaseStats,
  renderRecord,
  renderRecordList,
  renderSeriesView,
  writeRecords,
  type AthleticDatabase,
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
        seeded = await writeRecords(database, [
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
              database_revision: seeded.database_revision,
              benchmark_record_ids: seeded.records
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
      console.log(`  seeded ${SEED_BENCHMARKS.length} benchmarks: ${SEED_BENCHMARKS.map((b) => b.id).join(", ")}`);
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
    .description("load an Apple Health, WHOOP, or Oura export into the file")
    .addHelpText("after", EXAMPLES.import!)
    .argument("<path>", "the export: a zip, a folder, an export.xml, or a CSV")
    .option("--file <path>", "import into a file other than the one in this folder")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (exportPath: string, opts: { file?: string; json?: boolean }) => {
      const athletePath = findOrFail(opts.file);
      let file: AthleticStandardFileT;
      try {
        file = loadFile(athletePath);
      } catch (e) {
        return fail((e as Error).message);
      }

      // The bar draws on stderr and only when a person is watching. Piped output and
      // tests get the summary alone.
      const progress = process.stderr.isTTY && !opts.json ? terminalProgress() : silentProgress;
      let result;
      try {
        result = await importExport(file, athletePath, resolve(process.cwd(), exportPath), progress);
      } catch (e) {
        progress.finish();
        if (e instanceof UnknownExportError) return fail((e as Error).message);
        if (e instanceof PooledFileError) return fail((e as Error).message);
        return fail(
          `could not read that export: ${(e as Error).message}\n` +
            `  Point at the zip as it was downloaded, the folder it unzips to, an export.xml, ` +
            `or a vendor CSV. Nothing was written.`,
        );
      }

      // Refuse to write a file the import would have made invalid.
      const validation = validateAthleticStandardFile(file);
      if (!validation.valid) {
        const first = validation.issues.filter((i) => i.severity === "error").slice(0, 5);
        return fail(
          `import would produce an invalid file, so nothing was written:\n` +
            first.map((i) => `  ${i.path}: ${i.message}`).join("\n") +
            `\nRun \`ath check\` to see the file as it stands. This is a bug in the importer, ` +
            `not something you did.`,
        );
      }

      saveFile(athletePath, file);

      // A result with no session is a match still waiting. Device data usually lands
      // days after the workout was logged, so every import asks again about the ones
      // that have become possible (D53).
      const waiting = waitingMatches(file);

      if (opts.json) {
        console.log(
          JSON.stringify(
            {
              ...mergeSummaryAsJson(result.summary, result.label),
              waiting_matches: waiting.map((m) => ({
                benchmark: m.result.benchmark,
                recorded_at: m.result.recorded_at,
                candidates: m.candidates,
              })),
            },
            null,
            2,
          ),
        );
        return;
      }

      console.log(renderMergeSummary(result.summary, result.label));
      if (waiting.length === 0) return;

      const linked = await offerWaitingMatches(waiting);
      if (linked > 0) {
        const check = validateAthleticStandardFile(file);
        if (!check.valid) {
          return fail(
            `that link would make the file invalid, so nothing was written. ` +
              `Run \`ath check\` to see what is wrong with the file first.`,
          );
        }
        saveFile(athletePath, file);
      }
    });

  program
    .command("log")
    .helpGroup(GROUPS.getIn)
    .description("write something down: a workout result, a measurement, or how you felt")
    .addHelpText("after", EXAMPLES.log!)
    .argument("[entry...]", "the entry, unquoted. Leave it off to paste one, ending with Ctrl-D")
    .option("--benchmark <name>", "name the workout yourself, instead of naming it after the day")
    .option("--date <date>", "the day it happened, as YYYY-MM-DD. Leave it off for today")
    .option("--scaling <rx|scaled>", "whether the workout was done as written, or scaled")
    .option("--file <path>", "write to a file other than the one in this folder")
    .option("-y, --yes", "write it without asking — for scripts, and when you are sure")
    .option("--again", "record a second result on a day that already has one")
    .option("--dry-run", "show what would be written, and write nothing")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (words: string[], opts: LogOptions) => {
      const path = findOrFail(opts.file);
      let file: AthleticStandardFileT;
      try {
        file = loadFile(path);
      } catch (e) {
        return fail((e as Error).message);
      }

      // No words on the line means the entry is being pasted. A workout runs to several
      // lines and often carries a `"` for the box height, and a shell breaks on both,
      // so reading the paste is the only way to take one unquoted (D58).
      const text = words.length > 0 ? words.join(" ") : await readStdin();
      if (text.trim() === "") {
        return fail(
          `nothing to log. Type it on the line — \`ath log slept badly, about 5 hours\` — or run ` +
            `\`ath log\` on its own and paste it, ending with Ctrl-D.`,
        );
      }

      let draft;
      try {
        draft = buildDraft(file, {
          text,
          benchmark: opts.benchmark,
          date: opts.date,
          scaling: opts.scaling,
          again: opts.again,
          confirm: whoAnswers(opts),
        });
      } catch (e) {
        if (e instanceof LogRefusal) return fail((e as Error).message);
        throw e;
      }

      if (draft.blocks.length === 0) {
        return fail(
          `nothing in that could be logged. Try a shorter entry — ` +
            `\`ath log slept badly, about 5 hours\` — and see \`ath log --help\` for what it reads.`,
        );
      }

      // The summary, then one question. Every guess the tool made is on the screen
      // before anything reaches the file (D56).
      if (!opts.json) console.log(renderDraft(draft));

      if (opts.dryRun) {
        // The question is shown rather than asked, so a preview shows the whole shape
        // including the extra key a second candidate session adds.
        if (opts.json) console.log(JSON.stringify(draftAsJson(draft, false), null, 2));
        else console.log(`\n${renderQuestion(draft)}\nnothing written (--dry-run)`);
        return;
      }

      if (!opts.yes && !opts.json && process.stdout.isTTY) {
        const answer = await askOnTerminal(`\n${renderQuestion(draft)} `);
        if (answer === null) {
          return fail(
            `there is no terminal to ask on, so nothing was written. ` +
              `Pass --yes to write it without the question, or --dry-run to see what it would be.`,
          );
        }
        const choice = Number(answer);
        if (Number.isInteger(choice) && choice >= 2 && choice <= draft.candidates.length) {
          draft.chosenCandidate = choice - 1;
        } else if (!/^y(es)?$/i.test(answer)) {
          console.log("nothing written");
          return;
        }
      }

      try {
        applyDraft(file, draft);
      } catch (e) {
        if (e instanceof LogRefusal) return fail((e as Error).message);
        throw e;
      }

      const validation = validateAthleticStandardFile(file);
      if (!validation.valid) {
        const first = validation.issues.filter((i) => i.severity === "error").slice(0, 5);
        return fail(
          `that would make the file invalid, so nothing was written:\n` +
            first.map((i) => `  ${i.path}: ${i.message}`).join("\n"),
        );
      }

      saveFile(path, file);
      console.log(opts.json ? JSON.stringify(draftAsJson(draft, true), null, 2) : `\n${renderWritten(draft)}`);
    });

  program
    .command("link")
    .helpGroup(GROUPS.getIn)
    .description("attach a workout result to the device session it happened in")
    .addHelpText("after", EXAMPLES.link!)
    .argument("<result>", "the benchmark, e.g. `fran`, or `fran@2026-09-04` when there are several")
    .argument("<session>", "the session's start: `17:25`, a full timestamp, or `whoop-1@<timestamp>`")
    .option("--file <path>", "change a file other than the one in this folder")
    .option("--json", "structured output, for an agent rather than a person")
    .action((result: string, session: string, opts: { file?: string; json?: boolean }) => {
      const path = findOrFail(opts.file);
      let file: AthleticStandardFileT;
      try {
        file = loadFile(path);
      } catch (e) {
        return fail((e as Error).message);
      }

      let outcome;
      try {
        outcome = linkResult(file, result, session);
      } catch (e) {
        if (e instanceof LogRefusal) return fail((e as Error).message);
        throw e;
      }

      const validation = validateAthleticStandardFile(file);
      if (!validation.valid) {
        return fail(
          `that link would make the file invalid, so nothing was written. ` +
            `Run \`ath check\` to see what is wrong with the file first.`,
        );
      }
      saveFile(path, file);

      if (opts.json) {
        console.log(JSON.stringify(outcome, null, 2));
        return;
      }
      console.log(
        `${outcome.benchmark} on ${outcome.recordedAt.slice(0, 10)} is now linked to the ` +
          `${outcome.session.start.slice(11, 16)} session on ${outcome.session.source}` +
          (outcome.replaced ? ` (was ${outcome.replaced.start.slice(11, 16)} on ${outcome.replaced.source})` : ""),
      );
    });

  program
    .command("check")
    .helpGroup(GROUPS.read)
    .description("make sure the file still obeys every rule of the format")
    .addHelpText("after", EXAMPLES.check!)
    .argument("[file]", "the file to check (default: the one in this folder)")
    .option("--file <path>", "check a database other than the one in this folder")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (fileArg, opts: { file?: string; json?: boolean }) => {
      const path = findOrFail(opts.file ?? fileArg);
      const database = await openOrFail(path, opts.json);
      try {
        const result = await checkDatabase(database);
        if (opts.json) {
          console.log(JSON.stringify({ file: path, ...result }, null, 2));
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
    .option("--as-of <date>", "pretend it is this day, hiding everything after it. Does not write. Replay the past with ath backtest (YYYY-MM-DD)")
    .option("--file <path>", "read a file other than the one in this folder")
    .option("-y, --yes", "write the prediction without asking")
    .option("--dry-run", "show the prediction, and write nothing")
    .option("--json", "evidence for a harness, not a prediction")
    .action(async (benchmarkId: string, opts: PredictCliOptions) => {
      const path = findOrFail(opts.file);
      let file: AthleticStandardFileT;
      try {
        file = loadFile(path);
      } catch (e) {
        return fail((e as Error).message);
      }

      const asOf = opts.asOf ?? today();
      if (!/^\d{4}-\d{2}-\d{2}$/.test(asOf) || Number.isNaN(Date.parse(`${asOf}T00:00:00Z`))) {
        return fail(`--as-of takes a day written as YYYY-MM-DD, not '${asOf}'.`);
      }

      let benchmark;
      try {
        benchmark = benchmarkOrRefuse(file, benchmarkId);
      } catch (e) {
        if (e instanceof PredictRefusal) return fail((e as Error).message);
        throw e;
      }

      // --json is evidence for a harness. It is not a prediction, and it needs no key.
      if (opts.json) {
        const evidence = evidenceFor(file, path, benchmark, asOf);
        console.log(JSON.stringify(evidenceAsJson(evidence), null, 2));
        return;
      }

      let plan;
      try {
        plan = await planPredict(file, path, benchmark, {
          asOf,
          asOfPassed: opts.asOf !== undefined,
          ...(opts.model ? { model: opts.model } : {}),
          ...(opts.gateway ? { gateway: parseGateway(opts.gateway) } : {}),
          ...(opts.dryRun ? { dryRun: true } : {}),
        });
      } catch (e) {
        if (e instanceof PredictRefusal || e instanceof KeyRefusal) return fail((e as Error).message);
        throw e;
      }

      console.log(renderPrediction(plan));

      if (!plan.canWrite) {
        console.log(`\n${plan.holdReason}`);
        return;
      }

      if (!opts.yes && process.stdout.isTTY) {
        const answer = await askOnTerminal(`\nSave this?\n  [y] yes\n  [n] no\n`);
        if (answer === null) {
          return fail(
            `there is no terminal to ask on, so nothing was written. ` +
              `Pass --yes to write it without the question, or --dry-run to see what it would be.`,
          );
        }
        if (!/^y(es)?$/i.test(answer)) {
          console.log("nothing written");
          return;
        }
      } else if (!opts.yes) {
        return fail(
          `there is no terminal to ask on, so nothing was written. ` +
            `Pass --yes to write it without the question, or --dry-run to see what it would be.`,
        );
      }

      applyDraft(file, plan.draft);
      const validation = validateAthleticStandardFile(file);
      if (!validation.valid) {
        const first = validation.issues.filter((i) => i.severity === "error").slice(0, 5);
        return fail(
          `that would make the file invalid, so nothing was written:\n` +
            first.map((i) => `  ${i.path}: ${i.message}`).join("\n"),
        );
      }
      saveFile(path, file);
      console.log(`\n${renderWritten(plan.draft)}`);
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
      let file: AthleticStandardFileT;
      try {
        file = loadFile(path);
      } catch (e) {
        return fail((e as Error).message);
      }

      let plan;
      try {
        plan = await planBacktest(file, path, {
          ...(opts.model && opts.model.length > 0 ? { models: opts.model } : {}),
          ...(opts.all ? { all: true } : {}),
          ...(opts.gateway ? { gateway: parseGateway(opts.gateway) } : {}),
        });
      } catch (e) {
        if (e instanceof PredictRefusal || e instanceof KeyRefusal) return fail((e as Error).message);
        throw e;
      }

      if (opts.all && !opts.yes) {
        const calls = plan.models.length * plan.targets.length;
        const question =
          `This will call ${plan.models.length} models on ${plan.targets.length} ` +
          `replayable results (${calls} calls). Continue? [y] yes  [n] no `;
        if (!process.stdout.isTTY) {
          return fail(
            `this will call ${plan.models.length} models on ${plan.targets.length} results. ` +
              `Pass --yes to run it without the question.`,
          );
        }
        const answer = await askOnTerminal(question);
        if (answer === null || !/^y(es)?$/i.test(answer ?? "")) {
          console.log("nothing run");
          return;
        }
      }

      const before = loadFile(path);
      const report = await runBacktest(file, path, plan);
      const after = JSON.stringify(loadFile(path));
      if (after !== JSON.stringify(before)) {
        return fail(`backtest would have changed the athlete file, so the report was not written. This is a bug.`);
      }

      const reportPath = writeReport(path, report, new Date());
      if (opts.json) {
        console.log(JSON.stringify({ report: reportPath, ...report }, null, 2));
        return;
      }
      console.log(renderReport(report, reportPath));
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
      const report = path ? latestReport(path) : latestReport(join(process.cwd(), "athlete.ath.json"));
      if (opts.json) {
        console.log(JSON.stringify({ built: false, latest_report: report ?? null }, null, 2));
        return;
      }
      console.log(shareRefusal(report));
    });

  program
    .command("grade")
    .helpGroup(GROUPS.predict)
    .description("record what actually happened, and score the prediction against it")
    .addHelpText("after", EXAMPLES.grade!)
    .argument("<benchmark>", "the benchmark that was attempted, e.g. `fran`")
    .option("--actual <score>", "what happened: `4:32` for a time, `245` for reps, `100kg` for a load")
    .option("--date <date>", "the day of the attempt, as YYYY-MM-DD. Leave it off for today")
    .option("--scaling <rx|scaled>", "whether the workout was done as written, or scaled")
    .option("--analysis <json>", "the agent's write-up of a miss, after it has read the dossier")
    .option("--file <path>", "change a file other than the one in this folder")
    .option("-y, --yes", "write it without asking — for scripts, and when you are sure")
    .option("--again", "record a second attempt on a day that already has a result")
    .option("--dry-run", "show what would be written, and write nothing")
    .option("--json", "structured output, for an agent rather than a person")
    .action(async (benchmarkId: string, opts: GradeCliOptions) => {
      const path = findOrFail(opts.file);
      let file: AthleticStandardFileT;
      try {
        file = loadFile(path);
      } catch (e) {
        return fail((e as Error).message);
      }

      let benchmark;
      try {
        benchmark = benchmarkOrRefuse(file, benchmarkId);
      } catch (e) {
        if (e instanceof PredictRefusal) return fail((e as Error).message);
        throw e;
      }

      if (opts.analysis === undefined && opts.actual === undefined) {
        return fail(
          `say what happened: \`ath grade ${benchmarkId} --actual 4:32\`. ` +
            `Use --analysis afterwards to write up a miss.`,
        );
      }

      // Two calls, in the order the grading procedure runs: the result and the grade
      // first, then the analysis an agent wrote after reading the dossier (D62).
      const confirm = whoAnswers(opts);
      try {
        const outcome = opts.actual
          ? planGrade(file, path, benchmark, opts.actual, {
              date: opts.date,
              scaling: opts.scaling,
              again: opts.again,
              confirm,
            })
          : null;

        // Grading writes, so it shows what it will write and asks once, the same as
        // `ath log`. The verdict comes after, because it is the consequence of the
        // write rather than something to agree to (D65).
        if (outcome && !opts.json) console.log(renderDraft(outcome.draft));

        if (opts.dryRun) {
          if (opts.json && outcome) console.log(JSON.stringify(gradeAsJson(outcome, false), null, 2));
          else if (outcome) console.log(`\n${renderQuestion(outcome.draft)}\nnothing written (--dry-run)`);
          else console.log("nothing written (--dry-run)");
          return;
        }

        if (outcome && confirm === "ask" && !opts.yes && !opts.json && process.stdout.isTTY) {
          const answer = await askOnTerminal(`\n${renderQuestion(outcome.draft)} `);
          if (answer === null) {
            return fail(
              `there is no terminal to ask on, so nothing was written. ` +
                `Pass --yes to write it without the question, or --dry-run to see what it would be.`,
            );
          }
          const choice = Number(answer);
          if (Number.isInteger(choice) && choice >= 2 && choice <= outcome.draft.candidates.length) {
            outcome.draft.chosenCandidate = choice - 1;
          } else if (!/^y(es)?$/i.test(answer)) {
            console.log("nothing written");
            return;
          }
        }

        const written =
          opts.analysis !== undefined
            ? analysisAsJson(attachAnalysis(file, benchmark, opts.analysis))
            : null;
        if (outcome) applyGrade(file, outcome);

        const validation = validateAthleticStandardFile(file);
        if (!validation.valid) {
          const first = validation.issues.filter((i) => i.severity === "error").slice(0, 5);
          return fail(
            `that would make the file invalid, so nothing was written:\n` +
              first.map((i) => `  ${i.path}: ${i.message}`).join("\n"),
          );
        }
        saveFile(path, file);

        if (opts.json) {
          console.log(
            JSON.stringify(
              { ...(outcome ? gradeAsJson(outcome, true) : {}), ...(written ?? {}) },
              null,
              2,
            ),
          );
          return;
        }
        if (outcome) console.log(`\n${renderGrade(outcome)}`);
        if (written) {
          console.log(
            `analysis attached to '${written.prediction}'` +
              ((written.miss_analysis as { unexplained: boolean }).unexplained ? ", marked unexplained" : ""),
          );
        }
      } catch (e) {
        if (e instanceof GradeRefusal) return fail((e as Error).message);
        if (e instanceof LogRefusal) return fail((e as Error).message);
        throw e;
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

/**
 * Offer each waiting match, and return how many were attached.
 *
 * With no terminal it prints the candidates and the command that links them, because
 * a match nobody was told about is a match nobody makes.
 */
async function offerWaitingMatches(waiting: ReturnType<typeof waitingMatches>): Promise<number> {
  const plural = waiting.length === 1 ? "result" : "results";
  console.log(
    `\n${waiting.length} ${plural} logged earlier now have a session on the same day:`,
  );

  let linked = 0;
  for (const { result, candidates } of waiting) {
    const day = result.recorded_at.slice(0, 10);
    const best = candidates[0]!;
    console.log(`  ${result.benchmark} on ${day} — ${best.label}`);

    if (!process.stdout.isTTY) {
      console.log(`    ath link ${result.benchmark}@${day} ${best.start.slice(11, 16)}`);
      continue;
    }

    const extra = candidates
      .slice(1, 4)
      .map((c, i) => `  [${i + 2}] the ${c.start.slice(11, 16)} one`)
      .join("");
    const answer = await askOnTerminal(`    link it? [y] yes  [n] no${extra} `);
    if (answer === null) return linked;
    const choice = Number(answer);
    const pick =
      Number.isInteger(choice) && choice >= 2 && choice <= candidates.length
        ? candidates[choice - 1]!
        : /^y(es)?$/i.test(answer)
          ? best
          : null;
    if (!pick) continue;
    result.session = { source: pick.source, start: pick.start };
    linked++;
  }
  return linked;
}

interface GradeCliOptions {
  actual?: string;
  date?: string;
  scaling?: "rx" | "scaled";
  analysis?: string;
  file?: string;
  yes?: boolean;
  again?: boolean;
  dryRun?: boolean;
  json?: boolean;
}

interface LogOptions {
  benchmark?: string;
  date?: string;
  scaling?: "rx" | "scaled";
  file?: string;
  yes?: boolean;
  again?: boolean;
  dryRun?: boolean;
  json?: boolean;
}

interface PredictCliOptions {
  model?: string;
  gateway?: string;
  asOf?: string;
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

function parsePositiveInteger(value: string): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`expected a positive whole number, not '${value}'`);
  }
  return parsed;
}

function parseGateway(value: string): "vercel" | "openrouter" {
  if (isGatewayName(value)) return value;
  fail(`unknown gateway '${value}'. Use vercel or openrouter.`);
}

/**
 * Who will answer the one question, which is what the session match turns on (D68).
 *
 * `--yes` is agreement in advance, not permission to choose between two real
 * sessions. `--dry-run` previews the interactive run, so it answers the same way a
 * terminal would. Everything else has nobody to ask.
 */
function whoAnswers(opts: { yes?: boolean; dryRun?: boolean; json?: boolean }): Confirm {
  if (opts.yes) return "assume";
  if (opts.dryRun || (process.stdout.isTTY === true && !opts.json)) return "ask";
  return "none";
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
