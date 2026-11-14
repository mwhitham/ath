/**
 * Freeze the public JavaScript API of the 2.1.0 `athleticstandard` package.
 *
 * Spec 2.1.1 section 11.2 requires every exported symbol to be enumerated before
 * the rebuild, then mapped to a retained implementation and a compatibility test.
 *
 * The script reads `package.json` `exports` and the `tsup.config.ts` entry map,
 * then uses the TypeScript compiler API to list every export of each module
 * entry point, including `export *` re-exports, renamed re-exports, and the
 * `export * as database` namespace. JSON-only subpaths and the `bin` entry are
 * listed with their target files.
 *
 * Run from the repository root:
 *
 *   node --import tsx native/tools/reference/public-api-inventory.ts
 *
 * Output: native/reference/public-api.json (2-space JSON, trailing newline),
 * sorted by entry point, then export name.
 */
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const outPath = join(root, "native", "reference", "public-api.json");

interface PackageJson {
  name: string;
  version: string;
  main?: string;
  types?: string;
  bin?: Record<string, string>;
  exports: Record<string, string | { types?: string; import?: string }>;
}

const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8")) as PackageJson;

/**
 * tsup's `entry` map names each dist file after its source. Read it from the
 * config text rather than importing the config, which would load tsup itself.
 */
function tsupEntries(): Record<string, string> {
  const text = readFileSync(join(root, "tsup.config.ts"), "utf8");
  const block = /entry:\s*\{([^}]*)\}/m.exec(text);
  if (block === null) throw new Error("tsup.config.ts has no entry map");
  const entries: Record<string, string> = {};
  for (const match of block[1]!.matchAll(/(\w+)\s*:\s*"([^"]+)"/g)) {
    entries[match[1]!] = match[2]!;
  }
  return entries;
}

/** `./dist/database.js` → the source file tsup builds it from. */
function sourceForDist(distPath: string, entries: Record<string, string>): string {
  const base = /dist\/([^/]+)\.(?:js|d\.ts)$/.exec(distPath)?.[1];
  const source = base === undefined ? undefined : entries[base];
  if (source === undefined) throw new Error(`no tsup entry builds ${distPath}`);
  return source;
}

const entries = tsupEntries();

interface ModuleEntryPoint {
  subpath: string;
  import_target: string;
  types_target: string | null;
  source_file: string;
}

const moduleEntryPoints: ModuleEntryPoint[] = [];
const jsonOnlySubpaths: {
  subpath: string;
  target: string;
  target_exists: boolean;
  content_type: string;
}[] = [];

for (const [subpath, target] of Object.entries(packageJson.exports)) {
  if (typeof target === "string") {
    jsonOnlySubpaths.push({
      subpath,
      target,
      target_exists: existsSync(join(root, target)),
      content_type: target.endsWith(".json")
        ? "JSON Schema document"
        : target.endsWith(".md")
          ? "Markdown skill file"
          : "file",
    });
    continue;
  }
  if (target.import === undefined) throw new Error(`export ${subpath} has no import target`);
  moduleEntryPoints.push({
    subpath,
    import_target: target.import,
    types_target: target.types ?? null,
    source_file: sourceForDist(target.import, entries),
  });
}

const configFile = ts.readConfigFile(join(root, "tsconfig.json"), (path) => ts.sys.readFile(path));
if (configFile.error) throw new Error(ts.flattenDiagnosticMessageText(configFile.error.messageText, "\n"));
const parsed = ts.parseJsonConfigFileContent(configFile.config, ts.sys, root);
const program = ts.createProgram({
  rootNames: moduleEntryPoints.map((entry) => join(root, entry.source_file)),
  options: { ...parsed.options, noEmit: true },
});
const checker = program.getTypeChecker();

const FULL = ts.TypeFormatFlags.NoTruncation | ts.TypeFormatFlags.UseAliasDefinedOutsideCurrentScope;

/**
 * A declaration's file relative to the repository. TypeScript's own library files
 * are named as `typescript/lib/<file>` so the pnpm store path does not leak in.
 */
function relativeSource(node: ts.Node): string {
  const path = relative(root, node.getSourceFile().fileName).split("\\").join("/");
  const library = /(?:^|\/)node_modules\/(?:.*\/node_modules\/)?(typescript\/lib\/[^/]+)$/.exec(path);
  return library?.[1] ?? path;
}

function sha256(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

/** Leading JSDoc of a declaration, if any, as plain text. */
function documentation(symbol: ts.Symbol): string | null {
  const text = ts.displayPartsToString(symbol.getDocumentationComment(checker)).trim();
  return text === "" ? null : text;
}

function isZodSchemaType(type: ts.Type): boolean {
  return (
    type.getProperty("_zod") !== undefined &&
    type.getProperty("safeParse") !== undefined &&
    type.getProperty("parse") !== undefined
  );
}

/**
 * Full compiler renderings longer than this are recorded by hash and length only.
 * Some expanded Zod types exceed 150,000 characters. Their source definitions are
 * kept in `initializer_text` or `declared_text`, and the hash detects any change.
 */
const MAX_INLINE_TYPE_TEXT = 2_000;

/** The text a consumer's compiler sees for a value or type, with its hash. */
function typeText(type: ts.Type, node: ts.Node) {
  const text = checker.typeToString(type, node, FULL);
  return {
    type_text: text.length <= MAX_INLINE_TYPE_TEXT ? text : null,
    type_text_sha256: sha256(text),
    type_text_length: text.length,
  };
}

function describeFunction(symbol: ts.Symbol, declaration: ts.Declaration) {
  const type = checker.getTypeOfSymbolAtLocation(symbol, declaration);
  return {
    kind: "function",
    signatures: type
      .getCallSignatures()
      .map((signature) => checker.signatureToString(signature, declaration, FULL)),
  };
}

function describeClass(symbol: ts.Symbol, declaration: ts.ClassDeclaration) {
  const staticType = checker.getTypeOfSymbolAtLocation(symbol, declaration);
  const instanceType = checker.getDeclaredTypeOfSymbol(symbol);
  const heritage = (declaration.heritageClauses ?? []).map((clause) => clause.getText());
  const members = checker
    .getPropertiesOfType(instanceType)
    .filter((member) => {
      const flags = member.valueDeclaration
        ? ts.getCombinedModifierFlags(member.valueDeclaration)
        : ts.ModifierFlags.None;
      return (flags & (ts.ModifierFlags.Private | ts.ModifierFlags.Protected)) === 0 &&
        !member.name.startsWith("#");
    })
    .map((member) => ({
      name: member.name,
      declared_in: member.valueDeclaration ? relativeSource(member.valueDeclaration) : null,
      type_text: checker.typeToString(
        checker.getTypeOfSymbolAtLocation(member, declaration),
        declaration,
        FULL,
      ),
    }))
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  return {
    kind: "class",
    heading: `class ${declaration.name?.getText() ?? symbol.name}${heritage.length > 0 ? ` ${heritage.join(" ")}` : ""}`,
    constructor_signatures: staticType
      .getConstructSignatures()
      .map((signature) => checker.signatureToString(signature, declaration, FULL)),
    public_instance_members: members,
  };
}

function describeVariable(symbol: ts.Symbol, declaration: ts.VariableDeclaration) {
  const type = checker.getTypeOfSymbolAtLocation(symbol, declaration);
  const declarationList = declaration.parent;
  const isConst =
    ts.isVariableDeclarationList(declarationList) &&
    (declarationList.flags & ts.NodeFlags.Const) !== 0;
  const zod = isZodSchemaType(type);
  return {
    kind: zod ? "const Zod schema" : isConst ? "other const" : "variable",
    declared_type_annotation: declaration.type?.getText() ?? null,
    initializer_text: declaration.initializer?.getText() ?? null,
    ...typeText(type, declaration),
    ...(type.getCallSignatures().length > 0
      ? {
          call_signatures: type
            .getCallSignatures()
            .map((signature) => checker.signatureToString(signature, declaration, FULL)),
        }
      : {}),
  };
}

function describeTypeAlias(declaration: ts.TypeAliasDeclaration) {
  const type = checker.getTypeAtLocation(declaration.name);
  return {
    kind: "type",
    declared_text: declaration.getText(),
    ...typeText(type, declaration),
  };
}

function describeInterface(declaration: ts.InterfaceDeclaration) {
  const type = checker.getTypeAtLocation(declaration.name);
  return {
    kind: "interface",
    declared_text: declaration.getText(),
    ...typeText(type, declaration),
  };
}

function describeEnum(declaration: ts.EnumDeclaration) {
  return { kind: "enum", declared_text: declaration.getText() };
}

/**
 * One exported name can carry several meanings. `PredictionData` is both a
 * Zod schema value and an inferred type, for example. Each meaning is listed.
 */
function describeMeanings(symbol: ts.Symbol): Record<string, unknown>[] {
  const meanings: Record<string, unknown>[] = [];
  for (const declaration of symbol.getDeclarations() ?? []) {
    const base = { source_file: relativeSource(declaration) };
    if (ts.isFunctionDeclaration(declaration)) {
      // Overloads share one symbol. Record the signatures once.
      if (meanings.some((meaning) => meaning.kind === "function")) continue;
      meanings.push({ ...describeFunction(symbol, declaration), ...base });
    } else if (ts.isClassDeclaration(declaration)) {
      meanings.push({ ...describeClass(symbol, declaration), ...base });
    } else if (ts.isVariableDeclaration(declaration)) {
      meanings.push({ ...describeVariable(symbol, declaration), ...base });
    } else if (ts.isTypeAliasDeclaration(declaration)) {
      meanings.push({ ...describeTypeAlias(declaration), ...base });
    } else if (ts.isInterfaceDeclaration(declaration)) {
      meanings.push({ ...describeInterface(declaration), ...base });
    } else if (ts.isEnumDeclaration(declaration)) {
      meanings.push({ ...describeEnum(declaration), ...base });
    } else if (ts.isSourceFile(declaration)) {
      meanings.push({ kind: "namespace", ...base });
    } else {
      meanings.push({ kind: ts.SyntaxKind[declaration.kind], ...base });
    }
  }
  return meanings;
}

/** How the entry point reaches the declaration: local, named re-export, or `export *`. */
function exportRoute(exportSymbol: ts.Symbol, entryFile: string) {
  const declaration = exportSymbol.getDeclarations()?.[0];
  if (declaration === undefined) return { route: "export *", via: null, imported_name: null };
  const file = relativeSource(declaration);
  if (ts.isExportSpecifier(declaration)) {
    const exportDeclaration = declaration.parent.parent;
    const moduleText =
      exportDeclaration.moduleSpecifier && ts.isStringLiteral(exportDeclaration.moduleSpecifier)
        ? exportDeclaration.moduleSpecifier.text
        : null;
    return {
      route: moduleText === null ? "local export list" : "named re-export",
      via: moduleText,
      imported_name: declaration.propertyName?.getText() ?? null,
    };
  }
  if (ts.isNamespaceExport(declaration)) {
    const exportDeclaration = declaration.parent;
    const moduleText =
      exportDeclaration.moduleSpecifier && ts.isStringLiteral(exportDeclaration.moduleSpecifier)
        ? exportDeclaration.moduleSpecifier.text
        : null;
    return { route: "export * as namespace", via: moduleText, imported_name: null };
  }
  if (file === entryFile) return { route: "local declaration", via: null, imported_name: null };
  return { route: "export *", via: exportStarSource(exportSymbol, entryFile), imported_name: null };
}

/** The module specifier of the `export * from` statement that supplies a symbol. */
function exportStarSource(exportSymbol: ts.Symbol, entryFile: string): string | null {
  const sourceFile = program.getSourceFile(join(root, entryFile));
  if (sourceFile === undefined) return null;
  for (const statement of sourceFile.statements) {
    if (
      !ts.isExportDeclaration(statement) ||
      statement.exportClause !== undefined ||
      statement.moduleSpecifier === undefined ||
      !ts.isStringLiteral(statement.moduleSpecifier)
    ) {
      continue;
    }
    const target = checker.getSymbolAtLocation(statement.moduleSpecifier);
    if (target === undefined) continue;
    if (checker.getExportsOfModule(target).some((candidate) => candidate === exportSymbol)) {
      return statement.moduleSpecifier.text;
    }
  }
  return null;
}

function compareNames(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

const symbols: Record<string, unknown>[] = [];
const counts: Record<string, Record<string, number>> = {};

for (const entry of moduleEntryPoints) {
  const sourceFile = program.getSourceFile(join(root, entry.source_file));
  if (sourceFile === undefined) throw new Error(`cannot load ${entry.source_file}`);
  const moduleSymbol = checker.getSymbolAtLocation(sourceFile);
  if (moduleSymbol === undefined) throw new Error(`${entry.source_file} is not a module`);
  const exports = checker
    .getExportsOfModule(moduleSymbol)
    .slice()
    .sort((a, b) => compareNames(a.name, b.name));
  const entryCounts: Record<string, number> = { total: 0 };
  for (const exportSymbol of exports) {
    const resolved =
      (exportSymbol.flags & ts.SymbolFlags.Alias) !== 0
        ? checker.getAliasedSymbol(exportSymbol)
        : exportSymbol;
    const meanings = describeMeanings(resolved);
    const route = exportRoute(exportSymbol, entry.source_file);
    const isNamespace = meanings.some((meaning) => meaning.kind === "namespace");
    const record: Record<string, unknown> = {
      entry_point: entry.subpath,
      name: exportSymbol.name,
      // A namespace's own symbol name is its absolute module path, so it is not recorded.
      declared_name:
        resolved.name === exportSymbol.name || (resolved.flags & ts.SymbolFlags.ValueModule) !== 0
          ? null
          : resolved.name,
      kinds: [...new Set(meanings.map((meaning) => meaning.kind as string))],
      export_route: route.route,
      export_via: route.via,
      value_export: (resolved.flags & ts.SymbolFlags.Value) !== 0,
      type_export: (resolved.flags & ts.SymbolFlags.Type) !== 0,
      documentation: documentation(resolved),
      meanings,
    };
    if (isNamespace) {
      const members = checker
        .getExportsOfModule(resolved)
        .map((member) => member.name)
        .sort(compareNames);
      record.namespace_members = members;
      const matching = moduleEntryPoints.find(
        (other) => join(root, other.source_file) === resolved.valueDeclaration?.getSourceFile().fileName,
      );
      record.namespace_same_as_entry_point = matching?.subpath ?? null;
    }
    symbols.push(record);
    entryCounts.total = (entryCounts.total ?? 0) + 1;
    for (const kind of record.kinds as string[]) entryCounts[kind] = (entryCounts[kind] ?? 0) + 1;
  }
  counts[entry.subpath] = entryCounts;
}

symbols.sort(
  (a, b) =>
    compareNames(a.entry_point as string, b.entry_point as string) ||
    compareNames(a.name as string, b.name as string),
);

const inventory = {
  generated_by: relative(root, fileURLToPath(import.meta.url)).split("\\").join("/"),
  package: {
    name: packageJson.name,
    version: packageJson.version,
    main: packageJson.main ?? null,
    types: packageJson.types ?? null,
    typescript_version: ts.version,
  },
  tsup_entries: entries,
  module_entry_points: moduleEntryPoints,
  json_only_subpaths: jsonOnlySubpaths,
  bin: Object.entries(packageJson.bin ?? {}).map(([command, target]) => ({
    command,
    target,
    source_file: sourceForDist(target, entries),
  })),
  notes: [
    "kinds lists every meaning of one exported name; a Zod schema constant and its inferred type often share a name.",
    `type_text is the compiler's full rendering, omitted (null) when longer than ${MAX_INLINE_TYPE_TEXT} characters. type_text_sha256 and type_text_length always describe the full rendering.`,
    "For a Zod schema constant, initializer_text is its source definition. For a type alias or interface, declared_text is its source declaration.",
    "The root entry re-exports every ./schema export and exposes ./database as the namespace `database`.",
  ],
  counts,
  symbols,
};

writeFileSync(outPath, `${JSON.stringify(inventory, null, 2)}\n`);
console.log(
  `wrote ${relative(root, outPath)}: ` +
    Object.entries(counts)
      .map(([subpath, count]) => `${subpath} ${count.total}`)
      .join(", "),
);
