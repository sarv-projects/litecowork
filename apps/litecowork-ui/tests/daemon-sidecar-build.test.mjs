import assert from 'node:assert/strict';
import { lstatSync, mkdirSync, mkdtempSync, rmSync, symlinkSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  daemonSidecarPaths,
  ensureDaemonSidecarDirectory,
  parseCargoTargetDirectory,
  parseRustHostTriple,
  resolveTargetTriple,
  validateTargetTriple,
} from '../scripts/build-daemon-sidecar.mjs';
import { tauriBuildArgs, tauriInvocation } from '../scripts/run-tauri.mjs';

const repositoryRoot = path.resolve(import.meta.dirname, '../../..');

test('sidecar build paths match Tauri target-suffixed input and platform executable name', () => {
  assert.deepEqual(
    daemonSidecarPaths(repositoryRoot, 'x86_64-unknown-linux-gnu'),
    {
      source: path.join(
        repositoryRoot,
        'target',
        'x86_64-unknown-linux-gnu',
        'release',
        'litecoworkd',
      ),
      destination: path.join(
        repositoryRoot,
        'apps',
        'litecowork-ui',
        'src-tauri',
        'binaries',
        'litecoworkd-x86_64-unknown-linux-gnu',
      ),
    },
  );

  assert.equal(
    path.basename(
      daemonSidecarPaths(repositoryRoot, 'x86_64-pc-windows-msvc').destination,
    ),
    'litecoworkd-x86_64-pc-windows-msvc.exe',
  );
});

test('target triple parsing and validation reject malformed or path-like targets', () => {
  assert.equal(
    parseRustHostTriple('rustc 1.98.1\nhost: x86_64-unknown-linux-gnu\nrelease: 1.98.1'),
    'x86_64-unknown-linux-gnu',
  );
  assert.equal(validateTargetTriple('aarch64-apple-darwin'), 'aarch64-apple-darwin');
  assert.throws(() => validateTargetTriple('../outside'), /target triple/i);
  assert.throws(() => parseRustHostTriple('rustc 1.98.1\nrelease: 1.98.1'), /host triple/i);
});

test('Tauri target is authoritative and a conflicting standalone override fails closed', () => {
  assert.equal(
    resolveTargetTriple(
      'x86_64-pc-windows-msvc',
      'x86_64-pc-windows-msvc',
      'x86_64-unknown-linux-gnu',
    ),
    'x86_64-pc-windows-msvc',
  );
  assert.throws(
    () => resolveTargetTriple('x86_64-pc-windows-msvc', 'aarch64-apple-darwin', 'linux'),
    /conflicts with Tauri target/i,
  );
  assert.equal(
    resolveTargetTriple(undefined, 'aarch64-apple-darwin', 'x86_64-unknown-linux-gnu'),
    'aarch64-apple-darwin',
  );
});

test('sidecar source follows Cargo metadata when a custom target directory is configured', () => {
  const customTargetDirectory = path.join(repositoryRoot, 'custom-cargo-target');
  const metadata = JSON.stringify({ target_directory: customTargetDirectory });
  assert.equal(parseCargoTargetDirectory(metadata), customTargetDirectory);
  assert.equal(
    daemonSidecarPaths(
      repositoryRoot,
      'x86_64-unknown-linux-gnu',
      parseCargoTargetDirectory(metadata),
    ).source,
    path.join(
      customTargetDirectory,
      'x86_64-unknown-linux-gnu',
      'release',
      'litecoworkd',
    ),
  );
  assert.throws(() => parseCargoTargetDirectory('{"target_directory":5}'), /target directory/i);
});

test('sidecar output rejects symlinked Tauri paths and ancestor escapes', (t) => {
  const tempRoot = mkdtempSync(path.join(os.tmpdir(), 'litecowork-sidecar-path-'));
  const externalRoot = mkdtempSync(path.join(os.tmpdir(), 'litecowork-sidecar-external-'));
  const outside = path.join(tempRoot, 'outside');
  const appsDirectory = path.join(tempRoot, 'apps');
  const apps = path.join(appsDirectory, 'litecowork-ui');
  const tauriRoot = path.join(apps, 'src-tauri');
  mkdirSync(outside, { recursive: true });
  mkdirSync(apps, { recursive: true });

  try {
    try {
      symlinkSync(outside, tauriRoot, 'dir');
    } catch (error) {
      if (['EPERM', 'EACCES', 'ENOSYS'].includes(error.code)) {
        t.skip(`directory symlinks are unavailable: ${error.code}`);
        return;
      }
      throw error;
    }
    assert.throws(() => ensureDaemonSidecarDirectory(tempRoot), /must be a real directory/i);
    rmSync(tauriRoot);

    mkdirSync(tauriRoot);
    symlinkSync(outside, path.join(tauriRoot, 'binaries'), 'dir');
    assert.throws(() => ensureDaemonSidecarDirectory(tempRoot), /must be a real directory/i);
    assert.equal(lstatSync(outside).isDirectory(), true);
    rmSync(path.join(tauriRoot, 'binaries'));

    const externalApps = path.join(externalRoot, 'apps');
    const externalTauriRoot = path.join(externalApps, 'litecowork-ui', 'src-tauri');
    mkdirSync(externalTauriRoot, { recursive: true });
    rmSync(appsDirectory, { recursive: true, force: true });
    symlinkSync(externalApps, appsDirectory, 'dir');
    assert.throws(
      () => ensureDaemonSidecarDirectory(tempRoot),
      /must stay inside the repository/i,
    );
    assert.equal(lstatSync(path.join(externalTauriRoot, 'binaries'), { throwIfNoEntry: false }),
      undefined);
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
    rmSync(externalRoot, { recursive: true, force: true });
  }
});

test('Tauri production builds prepare and bundle the daemon sidecar', async () => {
  const config = JSON.parse(
    await readFile(
      path.join(repositoryRoot, 'apps', 'litecowork-ui', 'src-tauri', 'tauri.conf.json'),
      'utf8',
    ),
  );
  const bundleConfig = JSON.parse(
    await readFile(
      path.join(repositoryRoot, 'apps', 'litecowork-ui', 'src-tauri', 'tauri.bundle.conf.json'),
      'utf8',
    ),
  );
  const packageJson = JSON.parse(
    await readFile(path.join(repositoryRoot, 'apps', 'litecowork-ui', 'package.json'), 'utf8'),
  );

  assert.equal(config.bundle.externalBin, undefined);
  assert.match(config.build.beforeBuildCommand, /build:daemon-sidecar/);
  assert.equal(packageJson.scripts['build:daemon-sidecar'], 'node scripts/build-daemon-sidecar.mjs');
  assert.deepEqual(bundleConfig.bundle.externalBin, ['binaries/litecoworkd']);
  assert.deepEqual(
    tauriBuildArgs(['build', '--bundles', 'deb']),
    ['build', '--bundles', 'deb', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
  assert.deepEqual(
    tauriBuildArgs(['build'], 'linux'),
    ['build', '--bundles', 'deb', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
  assert.deepEqual(
    tauriBuildArgs(['build', '--bundles', 'deb'], 'linux'),
    ['build', '--bundles', 'deb', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
  assert.throws(() => tauriBuildArgs(['build', '--bundles', 'appimage'], 'linux'), /AppImage/i);
  assert.throws(() => tauriBuildArgs(['build', '--bundles', 'deb', 'appimage'], 'linux'), /AppImage/i);
  assert.throws(() => tauriBuildArgs(['build', '--bundles', 'all'], 'linux'), /AppImage/i);
  assert.throws(() => tauriBuildArgs(['build', '--bundles'], 'linux'), /select deb or rpm/i);
  assert.deepEqual(
    tauriBuildArgs(['build', '--target', 'x86_64-pc-windows-msvc'], 'linux'),
    [
      'build',
      '--target',
      'x86_64-pc-windows-msvc',
      '--config',
      'src-tauri/tauri.bundle.conf.json',
    ],
  );
  assert.deepEqual(
    tauriBuildArgs(['build'], 'win32'),
    ['build', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
  assert.deepEqual(tauriBuildArgs(['dev']), ['dev']);
  const invocation = tauriInvocation(['build', '--bundles', 'deb'], '/package/tauri.js');
  assert.equal(invocation.command, process.execPath);
  assert.deepEqual(invocation.args, [
    '/package/tauri.js',
    'build',
    '--bundles',
    'deb',
    '--config',
    'src-tauri/tauri.bundle.conf.json',
  ]);
});
