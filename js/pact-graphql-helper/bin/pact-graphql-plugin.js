#!/usr/bin/env node
'use strict';

// Thin CLI over the installer, so the plugin can be installed explicitly:
//   npx pact-graphql-plugin install [--force]
// Useful when the postinstall hook was skipped (`--ignore-scripts`, which many organisations
// set by default), or to repair or upgrade an existing installation.

const { installPlugin, PLUGIN_VERSION, defaultPluginRoot } = require('../dist/install');

const [command, ...rest] = process.argv.slice(2);

function usage() {
  console.log(`pact-graphql-plugin — installs the Pact GraphQL plugin binary

Usage:
  pact-graphql-plugin install [--force]

Options:
  --force     Reinstall even if a compatible plugin is already present
  --version   Print the plugin version this package installs
  --help      Show this message

Environment:
  PACT_PLUGIN_DIR                     Install root (default: ~/.pact/plugins)
  PACT_GRAPHQL_PLUGIN_REPOSITORY      GitHub repository to download releases from
  PACT_GRAPHQL_PLUGIN_SKIP_INSTALL    Set to skip the automatic postinstall
`);
}

async function main() {
  if (command === '--version' || command === '-v') {
    console.log(PLUGIN_VERSION);
    return;
  }

  if (!command || command === '--help' || command === '-h') {
    usage();
    return;
  }

  if (command !== 'install') {
    console.error(`unknown command "${command}"\n`);
    usage();
    process.exitCode = 1;
    return;
  }

  await installPlugin({
    force: rest.includes('--force'),
    pluginRoot: defaultPluginRoot(),
  });
}

main().catch((err) => {
  console.error(`pact-graphql-plugin: ${err.message}`);
  process.exitCode = 1;
});
