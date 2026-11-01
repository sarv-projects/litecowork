import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { assertSupportedNodeVersion } from '../scripts/check-node-version.mjs';

test('accepts stable Node 22.20.0 and above', () => {
  for (const version of ['22.20.0', '22.23.1', '24.21.0']) {
    assert.doesNotThrow(() => assertSupportedNodeVersion(version));
  }

  for (const version of ['20.10.0', '22.19.0', 'not-a-version']) {
    assert.throws(
      () => assertSupportedNodeVersion(version),
      /requires Node\.js >=22\.20\.0/,
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
