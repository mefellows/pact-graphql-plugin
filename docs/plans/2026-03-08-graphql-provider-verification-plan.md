# GraphQL Provider Verification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a provider-side verification example using Apollo v4 + Pact JS Verifier for the GraphQL plugin.

**Architecture:** A local Express + Apollo server uses the same SDL and resolvers as the consumer example. A separate verification script starts the server, runs Pact JS `Verifier` against the pact file, and shuts down cleanly.

**Tech Stack:** Node.js, Express, Apollo Server v4, Pact JS Verifier, TypeScript

---

### Task 0: Establish baseline for the JS example

**Files:**
- Verify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Build the helper package**

Run:
```bash
cd js/pact-graphql-helper
npm install
npm run build
```

Expected: `dist/` is created and imports resolve.

**Step 2: Run consumer tests**

Run:
```bash
cd examples/js/product-consumer
npm test
```

Expected: PASS and pact files generated under `examples/js/product-consumer/pacts`.

---

### Task 1: Add Apollo provider server

**Files:**
- Create: `examples/js/product-consumer/provider-server.ts`

**Step 1: Write a failing verification harness**

Create a minimal server file that exports a `startProviderServer()` returning `{ url, close }` but does not yet implement resolvers.

```ts
export async function startProviderServer() {
  throw new Error('not implemented');
}
```

**Step 2: Run provider verification script (will fail)**

Run:
```bash
npm run verify
```

Expected: FAIL with "not implemented".

**Step 3: Implement Apollo server**

Implement Express + `@apollo/server` with:
- SDL from `schema.graphql`
- resolvers for Query.product, Query.products, Query.search, Query.order
- Mutation.placeOrder, Mutation.addToCart, Mutation.submitReview
- Simple deterministic data for nested fields (variants, price, inventory, reviews)

**Step 4: Run consumer tests and verify**

Run:
```bash
npm test
npm run verify
```

Expected: PASS after resolver data matches pact.

**Step 5: Commit**

```bash
git add examples/js/product-consumer/provider-server.ts
git commit -m "feat: add Apollo provider for verification"
```

---

### Task 2: Add verification runner

**Files:**
- Create: `examples/js/product-consumer/verify-provider.ts`
- Modify: `examples/js/product-consumer/package.json`

**Step 1: Add verification script**

```ts
import { Verifier } from '@pact-foundation/pact';
import { startProviderServer } from './provider-server';

process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';

async function main() {
  const server = await startProviderServer();
  try {
    await new Verifier({
      provider: 'product-provider',
      providerBaseUrl: server.url,
      pactUrls: ['./pacts/product-consumer-product-provider.json'],
      logLevel: 'info',
    }).verifyProvider();
  } finally {
    await server.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

**Step 2: Add script entry**

Update `examples/js/product-consumer/package.json`:

```json
"scripts": {
  "test": "vitest run",
  "verify": "ts-node ./verify-provider.ts"
}
```

**Step 3: Run verification**

```bash
npm test
npm run verify
```

Expected: PASS.

**Step 4: Commit**

```bash
git add examples/js/product-consumer/verify-provider.ts examples/js/product-consumer/package.json
git commit -m "test: add provider verification script"
```

---

### Task 3: Align provider data with pact expectations

**Files:**
- Modify: `examples/js/product-consumer/provider-server.ts`

**Step 1: Add a failing test case**

Adjust the resolver payload to intentionally mismatch one field, run verification, confirm it fails.

**Step 2: Fix resolver data**

Make resolver outputs match the pact data for:
- product query
- nested products query
- placeOrder mutation

**Step 3: Run verification**

```bash
npm test
npm run verify
```

Expected: PASS.

**Step 4: Commit**

```bash
git add examples/js/product-consumer/provider-server.ts
git commit -m "test: align Apollo provider responses with pact"
```

---

## Verification

- `npm test` in `examples/js/product-consumer`
- `npm run verify` in `examples/js/product-consumer`
