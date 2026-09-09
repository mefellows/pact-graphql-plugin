#!/usr/bin/env node
'use strict';

// Best-effort plugin install after `npm install`.
//
// Deliberately never fails the install. A consumer's `npm install` breaking because GitHub was
// unreachable, or because the platform has no published build, would be a worse outcome than
// them running `npx pact-graphql-plugin install` themselves -- which the warning tells them to
// do. Skipped entirely when PACT_GRAPHQL_PLUGIN_SKIP_INSTALL is set, and a no-op in a source
// checkout where dist/ has not been built yet.

const { existsSync } = require('node:fs');
const { join } = require('node:path');

const skip = process.env.PACT_GRAPHQL_PLUGIN_SKIP_INSTALL;
if (skip && skip !== '0' && skip !== 'false') {
  process.exit(0);
}

const installer = join(__dirname, '..', 'dist', 'install.js');
if (!existsSync(installer)) {
  // Source checkout before a build; nothing to run.
  process.exit(0);
}

const { installPlugin, defaultPluginRoot } = require(installer);

installPlugin({ pluginRoot: defaultPluginRoot() }).catch((err) => {
  console.warn(
    `\npact-graphql-plugin: could not install the plugin automatically (${err.message}).\n` +
      'Run `npx pact-graphql-plugin install` to retry, or `just install` from a source checkout.\n',
  );
});
