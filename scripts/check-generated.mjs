import { execFileSync } from 'node:child_process';

const paths = ['schema', 'skills/athletic-standard/references/cli.md'];
try {
  if (!process.env.npm_execpath) throw new Error('Run this check through pnpm check:generated');
  for (const script of ['generate:json-schema', 'generate:skill-cli']) {
    // Invoke the current package manager through Node on every supported OS.
    execFileSync(process.execPath, [process.env.npm_execpath, 'run', script], { stdio: 'inherit' });
  }
  execFileSync('git', ['diff', '--exit-code', '--', ...paths], { stdio: 'inherit' });
  const untracked = execFileSync('git', ['ls-files', '--others', '--exclude-standard', '-z', '--', ...paths], { encoding: 'utf8' });
  if (untracked) throw new Error('Generated files are untracked; inspect and stage the generated outputs');
  console.log('Generated output check passed');
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
