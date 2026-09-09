import { describe, expect, it } from 'vitest';
import { gzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  installPlugin,
  isAlreadySatisfied,
  parseChecksum,
  releaseBaseUrl,
  resolveAsset,
} from '../src/install';

describe('resolveAsset', () => {
  // Names must match what `just bundle` uploads to the release, verified against v0.2.0.
  it.each([
    ['darwin', 'arm64', 'pact-graphql-plugin-macos-aarch64.gz', 'pact-graphql-plugin'],
    ['darwin', 'x64', 'pact-graphql-plugin-macos-x86_64.gz', 'pact-graphql-plugin'],
    ['linux', 'arm64', 'pact-graphql-plugin-linux-aarch64.gz', 'pact-graphql-plugin'],
    ['linux', 'x64', 'pact-graphql-plugin-linux-x86_64.gz', 'pact-graphql-plugin'],
    ['win32', 'x64', 'pact-graphql-plugin-windows-x86_64.exe.gz', 'pact-graphql-plugin.exe'],
    ['win32', 'arm64', 'pact-graphql-plugin-windows-aarch64.exe.gz', 'pact-graphql-plugin.exe'],
  ])('maps %s/%s to %s', (platform, arch, archive, binary) => {
    expect(resolveAsset(platform, arch)).toEqual({ archive, binary });
  });

  it('rejects a platform with no published build', () => {
    expect(() => resolveAsset('freebsd', 'x64')).toThrow(/freebsd/);
  });

  it('rejects an architecture with no published build', () => {
    expect(() => resolveAsset('linux', 'ia32')).toThrow(/ia32/);
  });
});

describe('parseChecksum', () => {
  it('takes the hash from shasum output, ignoring the path', () => {
    const line = '5f1dd728c6592c7dae523ddbe529e281bd5d39166e85b09c2c21d2f760699eee  dist/x/y.gz\n';
    expect(parseChecksum(line)).toBe(
      '5f1dd728c6592c7dae523ddbe529e281bd5d39166e85b09c2c21d2f760699eee',
    );
  });

  it('rejects output it cannot read a hash from', () => {
    expect(() => parseChecksum('not a checksum')).toThrow(/checksum/i);
  });
});

describe('releaseBaseUrl', () => {
  it('builds a download URL from the package repository', () => {
    expect(releaseBaseUrl('git+https://github.com/acme/thing.git', '1.2.3')).toBe(
      'https://github.com/acme/thing/releases/download/v1.2.3',
    );
  });

  it('accepts a plain https repository url', () => {
    expect(releaseBaseUrl('https://github.com/acme/thing', '1.2.3')).toBe(
      'https://github.com/acme/thing/releases/download/v1.2.3',
    );
  });

  it('rejects a repository it cannot derive a release from', () => {
    expect(() => releaseBaseUrl('git@gitlab.com:acme/thing.git', '1.2.3')).toThrow(/repository/i);
  });
});

describe('isAlreadySatisfied', () => {
  const withPlugins = (versions: string[]) => {
    const root = mkdtempSync(join(tmpdir(), 'pact-plugins-'));
    for (const v of versions) {
      const dir = join(root, `graphql-${v}`);
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, 'pact-plugin.json'), JSON.stringify({ name: 'graphql', version: v }));
    }
    return root;
  };

  it('is satisfied by the exact version', () => {
    expect(isAlreadySatisfied(withPlugins(['0.2.0']), '0.2.0')).toBe(true);
  });

  it('is satisfied by a newer version, since the driver takes the highest', () => {
    expect(isAlreadySatisfied(withPlugins(['0.5.1']), '0.2.0')).toBe(true);
  });

  it('is not satisfied by an older version', () => {
    expect(isAlreadySatisfied(withPlugins(['0.1.0']), '0.2.0')).toBe(false);
  });

  it('is not satisfied when nothing is installed', () => {
    expect(isAlreadySatisfied(withPlugins([]), '0.2.0')).toBe(false);
  });

  it('ignores unrelated plugins', () => {
    const root = withPlugins([]);
    mkdirSync(join(root, 'protobuf-1.0.0'), { recursive: true });
    expect(isAlreadySatisfied(root, '0.2.0')).toBe(false);
  });
});

describe('installPlugin', () => {
  const binary = Buffer.from('#!/bin/sh\necho plugin\n');
  const archive = gzipSync(binary);
  const digest = createHash('sha256').update(archive).digest('hex');
  const manifest = JSON.stringify({ name: 'graphql', version: '0.2.0' });

  const fakeFetch = (overrides: Record<string, unknown> = {}) =>
    (async (url: string) => {
      const body: Record<string, { buf: Buffer } | undefined> = {
        gz: { buf: archive },
        sha256: { buf: Buffer.from(`${digest}  dist/x/y.gz\n`) },
        json: { buf: Buffer.from(manifest) },
      };
      const key = url.endsWith('.sha256') ? 'sha256' : url.endsWith('.json') ? 'json' : 'gz';
      // `hasOwnProperty`, not `??`: an override of `undefined` means "this asset is
      // missing", which `??` would silently turn back into the real body.
      const entry = Object.prototype.hasOwnProperty.call(overrides, key)
        ? ((overrides as any)[key] as { buf: Buffer } | undefined)
        : body[key];
      if (!entry) return { ok: false, status: 404, statusText: 'Not Found' };
      return {
        ok: true,
        status: 200,
        arrayBuffer: async () =>
          entry.buf.buffer.slice(entry.buf.byteOffset, entry.buf.byteOffset + entry.buf.byteLength),
      };
    }) as any;

  const options = (root: string, extra: Record<string, unknown> = {}) => ({
    version: '0.2.0',
    repository: 'git+https://github.com/acme/thing.git',
    pluginRoot: root,
    platform: 'linux',
    arch: 'x64',
    fetch: fakeFetch(),
    log: () => {},
    ...extra,
  });

  it('writes the binary and manifest into a versioned plugin directory', async () => {
    const root = mkdtempSync(join(tmpdir(), 'pact-install-'));

    const result = await installPlugin(options(root));

    const dir = join(root, 'graphql-0.2.0');
    expect(result).toMatchObject({ installed: true, directory: dir });
    expect(readFileSync(join(dir, 'pact-graphql-plugin'))).toEqual(binary);
    expect(JSON.parse(readFileSync(join(dir, 'pact-plugin.json'), 'utf8')).version).toBe('0.2.0');
  });

  it('makes the binary executable', async () => {
    const root = mkdtempSync(join(tmpdir(), 'pact-install-'));
    await installPlugin(options(root));
    const { mode } = await import('node:fs').then((fs) =>
      fs.statSync(join(root, 'graphql-0.2.0', 'pact-graphql-plugin')),
    );
    expect(mode & 0o111).not.toBe(0);
  });

  it('refuses to install when the checksum does not match', async () => {
    const root = mkdtempSync(join(tmpdir(), 'pact-install-'));
    const tampered = fakeFetch({ sha256: { buf: Buffer.from(`${'0'.repeat(64)}  x.gz\n`) } });

    await expect(installPlugin(options(root, { fetch: tampered }))).rejects.toThrow(/checksum/i);
    // Nothing half-written is left behind for pact to pick up.
    expect(existsSync(join(root, 'graphql-0.2.0', 'pact-graphql-plugin'))).toBe(false);
  });

  it('reports a download failure with the asset that was missing', async () => {
    const root = mkdtempSync(join(tmpdir(), 'pact-install-'));
    const missing = fakeFetch({ gz: undefined });

    await expect(installPlugin(options(root, { fetch: missing }))).rejects.toThrow(
      /pact-graphql-plugin-linux-x86_64\.gz/,
    );
  });

  it('skips the download when a satisfying plugin is already installed', async () => {
    const root = mkdtempSync(join(tmpdir(), 'pact-install-'));
    mkdirSync(join(root, 'graphql-0.9.0'), { recursive: true });
    writeFileSync(
      join(root, 'graphql-0.9.0', 'pact-plugin.json'),
      JSON.stringify({ name: 'graphql', version: '0.9.0' }),
    );

    const result = await installPlugin(
      options(root, {
        fetch: () => {
          throw new Error('should not download');
        },
      }),
    );

    expect(result).toMatchObject({ installed: false, reason: 'already-installed' });
  });

  it('reinstalls when asked to, even if satisfied', async () => {
    const root = mkdtempSync(join(tmpdir(), 'pact-install-'));
    mkdirSync(join(root, 'graphql-0.9.0'), { recursive: true });
    writeFileSync(
      join(root, 'graphql-0.9.0', 'pact-plugin.json'),
      JSON.stringify({ name: 'graphql', version: '0.9.0' }),
    );

    const result = await installPlugin(options(root, { force: true }));

    expect(result).toMatchObject({ installed: true });
  });
});
