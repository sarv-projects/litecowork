import { spawnSync } from 'node:child_process';
import {
  chmodSync,
  copyFileSync,
  lstatSync,
  mkdirSync,
  realpathSync,
  statSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function validateTargetTriple(value) {
  if (typeof value !== 'string' || !/^[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+)+$/.test(value)) {
    throw new Error(`Invalid Rust target triple: ${String(value)}`);
  }
  return value;
}

export function parseRustHostTriple(output) {
  const match = /^host:\s*(\S+)\s*$/m.exec(output);
  if (!match) throw new Error('rustc -vV did not report a host triple');
  return validateTargetTriple(match[1]);
}

export function parseCargoTargetDirectory(output) {
  let metadata;
  try {
    metadata = JSON.parse(output);
  } catch {
    throw new Error('Cargo metadata did not return valid JSON');
  }
  if (typeof metadata.target_directory !== 'string' || metadata.target_directory.length === 0) {
    throw new Error('Cargo metadata did not report a target directory');
  }
  return path.resolve(metadata.target_directory);
}

export function resolveTargetTriple(tauriTarget, explicitTarget, hostTarget) {
  const selectedTauriTarget = tauriTarget ? validateTargetTriple(tauriTarget) : undefined;
  const selectedExplicitTarget = explicitTarget ? validateTargetTriple(explicitTarget) : undefined;
  if (
    selectedTauriTarget &&
    selectedExplicitTarget &&
    selectedTauriTarget !== selectedExplicitTarget
  ) {
    throw new Error('LITECOWORKD_TARGET conflicts with Tauri target');
  }
  if (selectedTauriTarget || selectedExplicitTarget) {
    return selectedTauriTarget ?? selectedExplicitTarget;
  }
  return validateTargetTriple(hostTarget);
}

export function daemonSidecarPaths(
  repositoryRoot,
  targetTriple,
  cargoTargetDirectory = path.join(repositoryRoot, 'target'),
) {
  const target = validateTargetTriple(targetTriple);
  const targetDirectory = path.resolve(cargoTargetDirectory);
  const executableSuffix = target.includes('windows') ? '.exe' : '';
  const executableName = `litecoworkd${executableSuffix}`;
  return {
    source: path.join(targetDirectory, target, 'release', executableName),
    destination: path.join(
      repositoryRoot,
      'apps',
      'litecowork-ui',
      'src-tauri',
      'binaries',
      `litecoworkd-${target}${executableSuffix}`,
    ),
  };
}

export function daemonBuildArgs(targetTriple) {
  return [
    'build',
    '--locked',
    '--release',
    '-p',
    'litecoworkd',
    '--target',
    validateTargetTriple(targetTriple),
  ];
}

export function ensureDaemonSidecarDirectory(repositoryRoot) {
  const canonicalRepositoryRoot = realpathSync(repositoryRoot);
  const tauriDirectory = path.join(
    repositoryRoot,
    'apps',
    'litecowork-ui',
    'src-tauri',
  );
  const tauriStat = lstatSync(tauriDirectory);
  if (!tauriStat.isDirectory() || tauriStat.isSymbolicLink()) {
    throw new Error('The Tauri source directory must be a real directory');
  }
  const canonicalTauriDirectory = realpathSync(tauriDirectory);
  const relativeTauriPath = path.relative(canonicalRepositoryRoot, canonicalTauriDirectory);
  if (
    path.isAbsolute(relativeTauriPath) ||
    relativeTauriPath === '..' ||
    relativeTauriPath.startsWith(`..${path.sep}`)
  ) {
    throw new Error('The Tauri source directory must stay inside the repository');
  }

  const binariesDirectory = path.join(tauriDirectory, 'binaries');
  try {
    mkdirSync(binariesDirectory);
  } catch (error) {
    if (error.code !== 'EEXIST') throw error;
  }
  const binariesStat = lstatSync(binariesDirectory);
  if (!binariesStat.isDirectory() || binariesStat.isSymbolicLink()) {
    throw new Error('The daemon sidecar directory must be a real directory');
  }
  if (path.dirname(realpathSync(binariesDirectory)) !== canonicalTauriDirectory) {
    throw new Error('The daemon sidecar directory must stay inside the Tauri source directory');
  }
  return binariesDirectory;
}

function rustHostTriple(repositoryRoot) {
  const result = spawnSync('rustc', ['-vV'], {
    cwd: repositoryRoot,
    encoding: 'utf8',
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`rustc -vV failed with status ${result.status}`);
  }
  return parseRustHostTriple(result.stdout);
}

function cargoTargetDirectory(repositoryRoot) {
  const result = spawnSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {
    cwd: repositoryRoot,
    encoding: 'utf8',
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`cargo metadata failed with status ${result.status}`);
  }
  return parseCargoTargetDirectory(result.stdout);
}

function prepareDaemonSidecar() {
  const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
  const repositoryRoot = path.resolve(scriptDirectory, '../../..');
  const tauriTarget = process.env.TAURI_ENV_TARGET_TRIPLE;
  const explicitTarget = process.env.LITECOWORKD_TARGET;
  const hostTarget = tauriTarget || explicitTarget ? undefined : rustHostTriple(repositoryRoot);
  const targetTriple = resolveTargetTriple(tauriTarget, explicitTarget, hostTarget);
  const build = spawnSync('cargo', daemonBuildArgs(targetTriple), {
    cwd: repositoryRoot,
    stdio: 'inherit',
  });
  if (build.error) throw build.error;
  if (build.status !== 0) {
    throw new Error(`Cargo daemon build failed with status ${build.status}`);
  }

  const targetDirectory = cargoTargetDirectory(repositoryRoot);
  const { source, destination } = daemonSidecarPaths(
    repositoryRoot,
    targetTriple,
    targetDirectory,
  );
  const sourceStat = statSync(source);
  if (!sourceStat.isFile() || sourceStat.size === 0) {
    throw new Error('The release daemon binary is missing or empty');
  }
  ensureDaemonSidecarDirectory(repositoryRoot);
  try {
    if (!lstatSync(destination).isFile()) {
      throw new Error('The daemon sidecar destination is not a regular file');
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  copyFileSync(source, destination);
  if (process.platform !== 'win32') chmodSync(destination, sourceStat.mode & 0o777);
  console.log(`Prepared LiteCowork daemon sidecar for ${targetTriple}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    prepareDaemonSidecar();
  } catch (error) {
    console.error(`Failed to prepare LiteCowork daemon sidecar: ${error.message}`);
    process.exitCode = 1;
  }
}
