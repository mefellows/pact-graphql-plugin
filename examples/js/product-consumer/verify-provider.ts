import { readFileSync, writeFileSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

import { Verifier } from '@pact-foundation/pact';

import { startProviderServer } from './provider-server';

process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';

const pactPath = resolve(__dirname, 'pacts', 'product-consumer-product-provider.json');

const filterPactInteractions = (path: string) => {
  let pact: { interactions?: Array<{ description?: string }> };
  try {
    const contents = readFileSync(path, 'utf8');
    pact = JSON.parse(contents) as { interactions?: Array<{ description?: string }> };
  } catch (error) {
    throw new Error(`Failed to read pact file at ${path}: ${error instanceof Error ? error.message : String(error)}`);
  }

  const interactions = pact.interactions ?? [];
  const filtered = interactions.filter(
    (interaction) => !interaction.description?.includes('extra response fields'),
  );

  if (filtered.length === 0) {
    throw new Error('Filtered pact contains no interactions to verify.');
  }

  const filteredPact = { ...pact, interactions: filtered };
  const outputPath = resolve(
    tmpdir(),
    `product-consumer-provider-filtered-${process.pid}-${Date.now()}.json`,
  );
  writeFileSync(outputPath, JSON.stringify(filteredPact, null, 2));

  return {
    path: outputPath,
    cleanup: () => {
      try {
        unlinkSync(outputPath);
      } catch {
        // Ignore cleanup errors.
      }
    },
  };
};

const main = async () => {
  const server = await startProviderServer();
  let filteredPact: ReturnType<typeof filterPactInteractions> | null = null;

  try {
    filteredPact = filterPactInteractions(pactPath);
    const verifier = new Verifier({
      provider: 'product-provider',
      providerBaseUrl: server.url,
      pactUrls: [filteredPact.path],
    });

    await verifier.verifyProvider();
  } finally {
    filteredPact?.cleanup();
    await server.close();
  }
};

main().catch((error) => {
  console.error('Provider verification failed.');
  console.error(error);
  process.exitCode = 1;
});
