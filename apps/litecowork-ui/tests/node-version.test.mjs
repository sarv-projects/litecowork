import assert from 'node:assert/strict';
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
