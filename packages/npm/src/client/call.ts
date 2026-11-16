/**
 * Call helpers for the public functions: closed-handle checks, the target
 * each request names, and streamed results.
 */
import type { AsyncOperationName, OperationTypes } from "../generated/protocol.js";
import type { AthleticDatabase } from "../types.js";
import { noteDatabaseId, targetOf } from "./handle.js";
import { callAsync, type CallOptions, type CallResult } from "./worker.js";

/** Run an operation on an open handle's database. */
export async function onDatabase<Name extends AsyncOperationName>(
  database: AthleticDatabase,
  name: Name,
  input: OperationTypes[Name]["input"],
  options: Omit<CallOptions<Name>, "target"> = {},
): Promise<CallResult<Name>> {
  const target = targetOf(database);
  const result = await callAsync(name, input, { ...options, target });
  noteDatabaseId(database, result.database_id);
  return result;
}

/** The output of an operation on an open handle's database. */
export async function outputOf<Name extends AsyncOperationName>(
  database: AthleticDatabase,
  name: Name,
  input: OperationTypes[Name]["input"],
): Promise<OperationTypes[Name]["output"]> {
  return (await onDatabase(database, name, input)).output;
}

/** The streamed items of an operation on an open handle's database. */
export async function itemsOf<Name extends AsyncOperationName>(
  database: AthleticDatabase,
  name: Name,
  input: OperationTypes[Name]["input"],
): Promise<Array<OperationTypes[Name]["item"]>> {
  return (await onDatabase(database, name, input)).items;
}

/** An object type whose possibly-undefined fields are optional instead. */
export type Defined<T> = {
  [K in keyof T as undefined extends T[K] ? never : K]: T[K];
} & {
  [K in keyof T as undefined extends T[K] ? K : never]?: Exclude<T[K], undefined>;
};

/** Drop `undefined` fields, as JSON would, so optional inputs stay optional. */
export function defined<T extends object>(value: T): Defined<T> {
  return Object.fromEntries(Object.entries(value).filter(([, entry]) => entry !== undefined)) as Defined<T>;
}
