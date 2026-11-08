import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

const script = resolve('scripts/check-decision-history.mjs');
const directories: string[] = [];
const path = 'build-history/v1.0.0/decisions.md';
const original = '# Decisions\n\n## D1. Keep the original\n\nReason.\n';

function repository() {
  const directory = mkdtempSync(join(tmpdir(), 'ath-history-'));
  directories.push(directory);
  const git = (...args: string[]) => execFileSync('git', args, { cwd: directory, encoding: 'utf8' }).trim();
  git('init', '-q');
  git('config', 'user.name', 'Synthetic Test');
  git('config', 'user.email', 'test@example.invalid');
  mkdirSync(join(directory, 'build-history/v1.0.0'), { recursive: true });
  writeFileSync(join(directory, path), original);
  git('add', '.');
  git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'Synthetic baseline');
  const base = git('rev-parse', 'HEAD');
  const run = (ref = base) => spawnSync(process.execPath, [script, ref], { cwd: directory, encoding: 'utf8' });
  return { directory, git, run };
}

afterEach(() => { for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true }); });

describe('decision history check through a real Git repository', () => {
  it('accepts unchanged history, appended decisions, and revisions to unmerged proposals', () => {
    const { directory, git, run } = repository();
    expect(run().status).toBe(0);
    writeFileSync(join(directory, path), original + '\n## D2. New proposal\n\nReason.\n');
    expect(run().status).toBe(0);
    git('add', '.');
    git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'Unmerged proposal');
    writeFileSync(join(directory, path), original + '\n## D2. Revised proposal\n\nBetter reason.\n');
    expect(run().status).toBe(0);
  });

  it('rejects a rewrite, a deletion, and a move of recorded history', () => {
    const { directory, git, run } = repository();
    writeFileSync(join(directory, path), original.replace('Reason.', 'Different reason.'));
    expect(run().stderr).toContain('rewritten');
    expect(run().status).toBe(1);
    rmSync(join(directory, path));
    expect(run().stderr).toContain('deleted or moved');
    writeFileSync(join(directory, 'build-history/v1.0.0/renamed.md'), original);
    git('add', '-A');
    expect(run().status).toBe(1);
  });

  it('checks numbering across version folders without freezing specs or progress', () => {
    const { directory, git, run } = repository();
    mkdirSync(join(directory, 'build-history/v2.0.0'));
    const next = join(directory, 'build-history/v2.0.0/decisions.md');
    writeFileSync(next, '## D2. New version\n');
    writeFileSync(join(directory, 'build-history/v1.0.0/progress.md'), 'A completed step.\n');
    git('add', '.');
    expect(run().status).toBe(0);
    writeFileSync(next, '## D1. Duplicate\n');
    expect(run().stderr).toContain('duplicate');
    writeFileSync(next, '## D3. Skipped number\n');
    expect(run().stderr).toContain('continue from D2');
  });

  it('fails closed for an unavailable base and does not change files', () => {
    const { directory, run } = repository();
    expect(run('missing-base').status).toBe(1);
    expect(readFileSync(join(directory, path), 'utf8')).toBe(original);
  });
});
