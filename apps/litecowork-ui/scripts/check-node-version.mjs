import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const MINIMUM = [24, 21, 0];

export function assertSupportedNodeVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  const actual = match?.slice(1).map(Number);
  const supported =
    actual &&
    actual[0] === 24 &&
    (actual[1] > MINIMUM[1] ||
      (actual[1] === MINIMUM[1] && actual[2] >= MINIMUM[2]));

  if (!supported) {
    throw new Error(
      `LiteCowork desktop requires Node.js >=24.21.0 <25; found ${version}.`,
    );
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    assertSupportedNodeVersion(process.versions.node);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
