import { createHash } from 'node:crypto';
import {
  chmodSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { gunzipSync } from 'node:zlib';

/**
 * The plugin release this package installs, and the minimum it needs at runtime.
 *
 * Not the version of this npm package: the two are released independently, and a change touching
 * only the DSL bumps npm without moving the plugin. This is the version of the *plugin* whose
 * GitHub release the binaries are pulled from.
 */
export const PLUGIN_VERSION = '0.2.0';

const PLUGIN_NAME = 'graphql';
const BINARY_STEM = 'pact-graphql-plugin';

/** Only the targets `just bundle-all` actually publishes. */
const PLATFORMS: Record<string, string> = {
  darwin: 'macos',
  linux: 'linux',
  win32: 'windows',
};

const ARCHITECTURES: Record<string, string> = {
  x64: 'x86_64',
  arm64: 'aarch64',
};

export interface ResolvedAsset {
  archive: string;
  binary: string;
}

export function resolveAsset(platform: string, arch: string): ResolvedAsset {
  const os = PLATFORMS[platform];
  if (!os) {
    throw new Error(
      `no GraphQL plugin build is published for platform "${platform}" ` +
        `(supported: ${Object.keys(PLATFORMS).join(', ')})`,
    );
  }

  const cpu = ARCHITECTURES[arch];
  if (!cpu) {
    throw new Error(
      `no GraphQL plugin build is published for architecture "${arch}" ` +
        `(supported: ${Object.keys(ARCHITECTURES).join(', ')})`,
    );
  }

  const exe = os === 'windows' ? '.exe' : '';
  return {
    archive: `${BINARY_STEM}-${os}-${cpu}${exe}.gz`,
    binary: `${BINARY_STEM}${exe}`,
  };
}

/** `shasum -a 256` prints `<hash>  <path>`; only the hash is of interest. */
export function parseChecksum(contents: string): string {
  const match = contents.trim().match(/^([0-9a-f]{64})\b/i);
  if (!match) {
    throw new Error(`could not read a SHA-256 checksum from: ${contents.trim().slice(0, 80)}`);
  }
  return match[1].toLowerCase();
}

export function releaseBaseUrl(repository: string, version: string): string {
  const match = repository.match(/github\.com[/:]([^/]+)\/([^/.]+)/);
  if (!match) {
    throw new Error(
      `cannot derive a GitHub release URL from repository "${repository}"; ` +
        'set PACT_GRAPHQL_PLUGIN_REPOSITORY to an https GitHub URL',
    );
  }
  return `https://github.com/${match[1]}/${match[2]}/releases/download/v${version}`;
}

function compareVersions(a: string, b: string): number {
  const parse = (v: string) => v.split('.').map((part) => Number.parseInt(part, 10) || 0);
  const [x, y] = [parse(a), parse(b)];
  for (let i = 0; i < Math.max(x.length, y.length); i += 1) {
    const diff = (x[i] ?? 0) - (y[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * True when a plugin at or above `required` is already present. The driver loads the highest
 * installed plugin at or above the version asked for, so a newer one is fine and re-downloading
 * would be wasted work.
 */
export function isAlreadySatisfied(pluginRoot: string, required: string): boolean {
  let entries: string[];
  try {
    entries = readdirSync(pluginRoot);
  } catch {
    return false;
  }

  return entries.some((entry) => {
    if (!entry.startsWith(`${PLUGIN_NAME}-`)) return false;
    const version = entry.slice(PLUGIN_NAME.length + 1);
    return compareVersions(version, required) >= 0;
  });
}

export function defaultPluginRoot(): string {
  return process.env.PACT_PLUGIN_DIR || join(homedir(), '.pact', 'plugins');
}

type FetchLike = (url: string) => Promise<{
  ok: boolean;
  status: number;
  statusText?: string;
  arrayBuffer(): Promise<ArrayBuffer>;
}>;

export interface InstallOptions {
  version?: string;
  repository?: string;
  pluginRoot?: string;
  platform?: string;
  arch?: string;
  force?: boolean;
  fetch?: FetchLike;
  log?: (message: string) => void;
}

export interface InstallResult {
  installed: boolean;
  directory: string;
  version: string;
  reason?: 'already-installed';
}

async function download(fetchImpl: FetchLike, url: string, asset: string): Promise<Buffer> {
  const response = await fetchImpl(url);
  if (!response.ok) {
    throw new Error(
      `failed to download ${asset}: ${response.status} ${response.statusText ?? ''}`.trim(),
    );
  }
  return Buffer.from(await response.arrayBuffer());
}

export async function installPlugin(options: InstallOptions = {}): Promise<InstallResult> {
  const version = options.version ?? PLUGIN_VERSION;
  const pluginRoot = options.pluginRoot ?? defaultPluginRoot();
  const platform = options.platform ?? process.platform;
  const arch = options.arch ?? process.arch;
  const log = options.log ?? ((message: string) => console.log(message));
  const fetchImpl = options.fetch ?? ((globalThis as any).fetch as FetchLike);
  const directory = join(pluginRoot, `${PLUGIN_NAME}-${version}`);

  if (!options.force && isAlreadySatisfied(pluginRoot, version)) {
    log(`pact GraphQL plugin >= ${version} is already installed in ${pluginRoot}`);
    return { installed: false, directory, version, reason: 'already-installed' };
  }

  if (typeof fetchImpl !== 'function') {
    throw new Error('global fetch is unavailable; Node 18 or newer is required to install');
  }

  // Prefers this package's own `repository` field, so a fork installs from its own releases
  // rather than upstream's. The publish job rewrites that field to the repository the release
  // was actually built in, so the two always agree.
  const repository =
    options.repository ??
    process.env.PACT_GRAPHQL_PLUGIN_REPOSITORY ??
    packageRepository() ??
    'https://github.com/mefellows/pact-graphql-plugin';

  const { archive, binary } = resolveAsset(platform, arch);
  const base = releaseBaseUrl(repository, version);

  log(`downloading pact GraphQL plugin ${version} (${archive})`);
  const [archiveBytes, checksumBytes, manifestBytes] = await Promise.all([
    download(fetchImpl, `${base}/${archive}`, archive),
    download(fetchImpl, `${base}/${archive}.sha256`, `${archive}.sha256`),
    download(fetchImpl, `${base}/pact-plugin.json`, 'pact-plugin.json'),
  ]);

  const expected = parseChecksum(checksumBytes.toString('utf8'));
  const actual = createHash('sha256').update(archiveBytes).digest('hex');
  if (actual !== expected) {
    throw new Error(
      `checksum mismatch for ${archive}: expected ${expected}, got ${actual}. ` +
        'The download was not what the release publishes; nothing has been installed.',
    );
  }

  // Staged next to the target and moved into place, so a failure part-way through cannot leave a
  // half-written plugin directory for pact to discover.
  const staging = `${directory}.tmp-${process.pid}`;
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  try {
    writeFileSync(join(staging, binary), gunzipSync(archiveBytes));
    if (!binary.endsWith('.exe')) {
      chmodSync(join(staging, binary), 0o755);
    }
    writeFileSync(join(staging, 'pact-plugin.json'), manifestBytes);

    rmSync(directory, { recursive: true, force: true });
    renameSync(staging, directory);
  } catch (err) {
    rmSync(staging, { recursive: true, force: true });
    throw err;
  }

  log(`installed pact GraphQL plugin ${version} to ${directory}`);
  return { installed: true, directory, version };
}

/** Reads this package's own repository field, so a fork installs from its own releases. */
export function packageRepository(): string | undefined {
  try {
    const pkg = JSON.parse(readFileSync(join(__dirname, '..', 'package.json'), 'utf8'));
    return typeof pkg?.repository?.url === 'string' ? pkg.repository.url : undefined;
  } catch {
    return undefined;
  }
}
