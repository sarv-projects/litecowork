import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { assertSupportedNodeVersion } from '../scripts/check-node-version.mjs';

test('accepts only stable Node 24 releases from 24.21.0 onward', () => {
  for (const version of ['24.21.0', '24.21.1', '24.30.0']) {
    assert.doesNotThrow(() => assertSupportedNodeVersion(version));
  }

  for (const version of ['22.23.1', '24.20.9', '25.0.0', '24.21.0-rc.1', 'not-a-version']) {
    assert.throws(
      () => assertSupportedNodeVersion(version),
      /requires Node\.js >=24\.21\.0 <25/,
      `expected Node.js ${version} to be rejected`,
    );
  }
});

test('approves only the lock-pinned esbuild build script', () => {
  const workspace = readFileSync(new URL('../pnpm-workspace.yaml', import.meta.url), 'utf8');
  const allowBuilds = workspace.match(/^allowBuilds:\n((?:  .+\n?)+)/m)?.[1];

  assert.ok(allowBuilds, 'an explicit dependency build allowlist is required');
  assert.deepEqual(allowBuilds.trim().split('\n'), ["'esbuild@0.25.12': true"]);
  assert.doesNotMatch(workspace, /^dangerouslyAllowAllBuilds:\s*true$/m);
});
