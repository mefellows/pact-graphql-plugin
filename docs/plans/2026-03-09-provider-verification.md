# Provider Verification as Vitest Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the provider verification script with a single vitest test that verifies HTTP and message pacts together, while isolating negative pacts under a different provider name.

**Architecture:** One vitest file starts the GraphQL provider server, runs an HTTP verifier against the main pact, and runs a message provider verification against the message pact in the same test. Negative pacts use a different provider name so they are never passed to verification.

**Tech Stack:** Vitest, @pact-foundation/pact (Verifier + MessageProviderPact), Apollo Server (provider), Node fs/path.

---

### Task 1: Update negative pact provider name

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Write the failing test**

No new test needed; this is a refactor to change the provider name for the negative pact so it no longer pollutes the main provider pact.

**Step 2: Run test to verify it fails**

Skip (no test change yet).

**Step 3: Write minimal implementation**

Change the `PactV4` instantiation in the "response not in schema" block to use a different provider name:

```ts
const pact = new PactV4({ consumer: 'product-consumer', provider: 'product-provider-negative' });
```

**Step 4: Run test to verify it passes**

Run: `npm test`

Expected: PASS

**Step 5: Commit**

```bash
git add examples/js/product-consumer/pact.test.ts
git commit -m "test: isolate negative graphql pacts"
```

### Task 2: Replace verify-provider script with vitest test

**Files:**
- Create: `examples/js/product-consumer/verify-provider.test.ts`
- Delete: `examples/js/product-consumer/verify-provider.ts`
- Modify: `examples/js/product-consumer/package.json`

**Step 1: Write the failing test**

Create `verify-provider.test.ts` with a single `it` that will run verification but initially points to a non-existent pact file to assert failure (temporary sanity check).

```ts
import { beforeAll, afterAll, describe, it, expect } from 'vitest';
import { resolve } from 'node:path';
import { MessageProviderPact, Verifier } from '@pact-foundation/pact';
import { startProviderServer } from './provider-server';

process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';

describe('GraphQL provider verification', () => {
  let server: Awaited<ReturnType<typeof startProviderServer>>;

  beforeAll(async () => {
    server = await startProviderServer();
  });

  afterAll(async () => {
    await server.close();
  });

  it('verifies HTTP and message pacts', async () => {
    const httpPactPath = resolve(__dirname, 'pacts', 'missing.json');
    const verifier = new Verifier({
      provider: 'product-provider',
      providerBaseUrl: server.url,
      pactUrls: [httpPactPath],
    });

    await expect(verifier.verifyProvider()).rejects.toThrow();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run verify-provider.test.ts`

Expected: FAIL because pact file is missing.

**Step 3: Write minimal implementation**

Replace the failing placeholder with real pact paths and add message verification. Final test body should:

```ts
  it('verifies HTTP and message pacts', async () => {
    const httpPactPath = resolve(__dirname, 'pacts', 'product-consumer-product-provider.json');
    const messagePactPath = resolve(
      __dirname,
      'pacts',
      'messages',
      'product-consumer-product-provider.json',
    );

    const httpVerifier = new Verifier({
      provider: 'product-provider',
      providerBaseUrl: server.url,
      pactUrls: [httpPactPath],
    });

    const messagePact = new MessageProviderPact({
      provider: 'product-provider',
      pactUrls: [messagePactPath],
      messageProviders: {
        'a GraphQL subscription event': async () => ({
          subscription: 'InventoryChanged',
          variables: { variantId: 'var-1' },
          data: {
            inventoryChanged: {
              quantity: 42,
              updatedAt: '2026-03-08T12:00:00Z',
            },
          },
        }),
      },
    });

    await httpVerifier.verifyProvider();
    await messagePact.verify();
  });
```

Delete the old script `verify-provider.ts`.

Update `package.json` `verify` script to:

```json
"verify": "vitest run verify-provider.test.ts"
```

**Step 4: Run test to verify it passes**

Run: `npm run verify`

Expected: PASS with both HTTP and message verification.

**Step 5: Commit**

```bash
git add examples/js/product-consumer/verify-provider.test.ts \
  examples/js/product-consumer/verify-provider.ts \
  examples/js/product-consumer/package.json
git commit -m "test: verify graphql provider via vitest"
```

### Task 3: Full suite sanity check

**Files:**
- Test: `examples/js/product-consumer/subscription-message.test.ts`
- Test: `examples/js/product-consumer/pact.test.ts`

**Step 1: Run the full test suite**

Run: `npm test`

Expected: PASS

**Step 2: Commit (if new fixes required)**

Only if additional fixes were required:

```bash
git add <files>
git commit -m "test: stabilize graphql provider verification"
```
