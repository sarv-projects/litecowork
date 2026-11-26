import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { tauriBuildArgs } from '../scripts/run-tauri.mjs';

test('Windows builds default to per-user NSIS and MSI installers', () => {
  assert.deepEqual(
    tauriBuildArgs(['build'], 'win32'),
    ['build', '--bundles', 'nsis', 'msi', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
});

test('Windows target builds default to the Windows installers from another host', () => {
  assert.deepEqual(
    tauriBuildArgs(['build', '--target', 'x86_64-pc-windows-msvc'], 'linux'),
    ['build', '--target', 'x86_64-pc-windows-msvc', '--bundles', 'nsis', 'msi', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
});

test('Windows accepts an explicit supported subset and rejects empty or unsupported bundle selections', () => {
  assert.deepEqual(
    tauriBuildArgs(['build', '--bundles', 'msi'], 'win32'),
    ['build', '--bundles', 'msi', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
  assert.throws(() => tauriBuildArgs(['build', '--bundles'], 'win32'), /select nsis or msi explicitly/);
  assert.throws(() => tauriBuildArgs(['build', '--bundles', 'all'], 'win32'), /support nsis and msi only/);
});

test('no-bundle Windows builds do not add installer targets', () => {
  assert.deepEqual(
    tauriBuildArgs(['build', '--no-bundle'], 'win32'),
    ['build', '--no-bundle', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
});

test('Linux builds keep their Debian-only default and reject AppImage or all', () => {
  assert.deepEqual(
    tauriBuildArgs(['build'], 'linux'),
    ['build', '--bundles', 'deb', '--config', 'src-tauri/tauri.bundle.conf.json'],
  );
  assert.throws(() => tauriBuildArgs(['build', '--bundles', 'appimage'], 'linux'), /AppImage bundles are disabled/);
  assert.throws(() => tauriBuildArgs(['build', '--bundles', 'all'], 'linux'), /AppImage bundles are disabled/);
});

test('Windows installer defaults are pinned in the Tauri configuration', async () => {
  const scriptPath = fileURLToPath(import.meta.url);
  const configPath = path.resolve(path.dirname(scriptPath), '../src-tauri/tauri.conf.json');
  const config = JSON.parse(await readFile(configPath, 'utf8'));
  assert.deepEqual(config.bundle.targets, ['nsis', 'msi', 'deb']);
  assert.equal(config.bundle.windows.webviewInstallMode.type, 'embedBootstrapper');
  assert.equal(config.bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(config.bundle.windows.wix.upgradeCode, 'e78dd61b-7205-5b2c-91f7-3000affa1659');
});
