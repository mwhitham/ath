import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const decisionPath = /^build-history\/[^/]+\/decisions\.md$/;
const headings = text => [...text.matchAll(/^## D(\d+)\b/gm)].map(match => Number(match[1]));

// Compare only committed decisions at the PR base. Draft decisions newly added by
// this branch may still be revised before merge. Specs and progress are not frozen.
export function decisionHistoryErrors(base, current) {
  const errors = [];
  const seen = new Map();
  const oldNumbers = new Set([...base.values()].flatMap(headings));
  const lastOld = Math.max(0, ...oldNumbers);
  const added = [];
  for (const [path, oldText] of base) {
    const text = current.get(path);
    if (text === undefined) errors.push(`${path}: recorded decisions were deleted or moved`);
    else if (!text.startsWith(oldText)) errors.push(`${path}: recorded decisions were rewritten; append a new decision`);
  }
  for (const [path, text] of current) {
    for (const number of headings(text)) {
      if (seen.has(number)) errors.push(`D${number}: duplicate in ${seen.get(number)} and ${path}`);
      seen.set(number, path);
      if (!oldNumbers.has(number)) added.push(number);
    }
  }
  const sorted = [...new Set(added)].sort((a, b) => a - b);
  for (let index = 0; index < sorted.length; index++) {
    if (sorted[index] !== lastOld + index + 1) {
      errors.push(`D${sorted[index]}: new decision numbers must continue from D${lastOld + 1}`);
      break;
    }
  }
  return errors;
}

export function checkDecisionHistory(baseRef, cwd = process.cwd()) {
  const git = args => execFileSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  // Resolve to a commit before using it in git object syntax; no shell interpolation.
  const base = git(['rev-parse', '--verify', '--end-of-options', `${baseRef}^{commit}`]).trim();
  const paths = git(['ls-tree', '-r', '--name-only', '-z', base]).split('\0').filter(path => decisionPath.test(path));
  const tracked = git(['ls-files', '-z']).split('\0').filter(path => decisionPath.test(path));
  const before = new Map(paths.map(path => [path, git(['show', `${base}:${path}`])]));
  const after = new Map();
  for (const path of tracked) {
    try { after.set(path, readFileSync(resolve(cwd, path), 'utf8')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return decisionHistoryErrors(before, after);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2).filter(arg => arg !== '--');
  if (args.length !== 1) {
    console.error('Usage: node scripts/check-decision-history.mjs <base-commit-or-ref>');
    process.exitCode = 1;
  } else {
    try {
      const errors = checkDecisionHistory(args[0]);
      if (errors.length) throw new Error(errors.join('\n'));
      console.log('Decision history check passed');
    } catch (error) {
      console.error(error.message);
      process.exitCode = 1;
    }
  }
}
