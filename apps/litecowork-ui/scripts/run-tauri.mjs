import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function tauriBuildArgs(args, hostPlatform = process.platform) {
  if (args[0] !== 'build') return [...args];
  const result = [...args];
  const target = targetArgument(args);
  const targetPlatform = target ? platformForRustTarget(target) ?? hostPlatform : hostPlatform;
  if (targetPlatform === 'linux' && !args.includes('--no-bundle')) {
    const requested = bundleArguments(args);
    if (requested.present) {
      if (requested.values.length === 0) {
        throw new Error('Linux bundles must select deb or rpm explicitly');
      }
      if (requested.values.includes('appimage') || requested.values.includes('all')) {
        throw new Error(
          'Linux AppImage bundles are disabled until the daemon service executable path is persistent; select deb or rpm',
        );
      }
    } else {
      result.push('--bundles', 'deb');
    }
  }
  return [...result, '--config', 'src-tauri/tauri.bundle.conf.json'];
}

function targetArgument(args) {
  for (let index = 0; index < args.length; index += 1) {
    if ((args[index] === '--target' || args[index] === '-t') && args[index + 1]) {
      return args[index + 1];
    }
    if (args[index].startsWith('--target=')) return args[index].slice('--target='.length);
  }
  return undefined;
}

function platformForRustTarget(target) {
  if (target.includes('linux')) return 'linux';
  if (target.includes('windows')) return 'win32';
  if (target.includes('darwin')) return 'darwin';
  return undefined;
}

function bundleArguments(args) {
  const values = [];
  let present = false;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--bundles' || argument === '-b') {
      present = true;
      for (index += 1; index < args.length && !args[index].startsWith('-'); index += 1) {
        values.push(...args[index].split(',').map((value) => value.toLowerCase()));
      }
      index -= 1;
    } else if (argument.startsWith('--bundles=')) {
      present = true;
      values.push(...argument.slice('--bundles='.length).split(',').map((value) => value.toLowerCase()));
    }
  }
  return { present, values };
}

export function tauriInvocation(
  args,
  cliEntryPath = createRequire(import.meta.url).resolve('@tauri-apps/cli/tauri.js'),
) {
  return {
    command: process.execPath,
    args: [cliEntryPath, ...tauriBuildArgs(args)],
  };
}

function runTauri() {
  const invocation = tauriInvocation(process.argv.slice(2));
  const result = spawnSync(invocation.command, invocation.args, { stdio: 'inherit' });
  if (result.error) throw result.error;
  process.exitCode = result.status ?? 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    runTauri();
  } catch (error) {
    console.error(`Failed to run Tauri: ${error.message}`);
    process.exitCode = 1;
  }
}
