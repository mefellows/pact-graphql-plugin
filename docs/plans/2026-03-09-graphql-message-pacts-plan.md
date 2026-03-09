# GraphQL Message Pacts Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add message-pact support for GraphQL subscription events and remove HTTP DSL duplication via a new helper API.

**Architecture:** Introduce `graphqlHttpInteraction` and `graphqlMessageInteraction` in the JS helper. Update HTTP example tests to use the new helper and add a message-pact test for subscription events.

**Tech Stack:** Node.js, Pact JS V4, pact-graphql-helper, GraphQL SDL

---

### Task 1: Add HTTP helper wrapper

**Files:**
- Modify: `js/pact-graphql-helper/src/index.ts`
- Modify: `js/pact-graphql-helper/src/types.ts`
- Modify: `js/pact-graphql-helper/test/graphql-interaction.spec.ts`

**Step 1: Write failing helper tests**

Add tests for `graphqlHttpInteraction` that assert it:
- Applies `content-type: application/graphql`
- Injects `pluginContents` built from `{ schema, query, variables, operationName }`

**Step 2: Run tests to see failures**

Run:
```bash
cd js/pact-graphql-helper
npm test
```
Expected: FAIL (helper not implemented).

**Step 3: Implement helper**

Implement `graphqlHttpInteraction` that wraps `interaction.withRequest` and injects plugin contents from a shared formatter.

**Step 4: Run tests**

Run: `npm test`
Expected: PASS.

**Step 5: Commit**

```bash
git add js/pact-graphql-helper/src/index.ts js/pact-graphql-helper/src/types.ts js/pact-graphql-helper/test/graphql-interaction.spec.ts
git commit -m "feat: add graphql HTTP helper wrapper"
```

---

### Task 2: Add message helper

**Files:**
- Modify: `js/pact-graphql-helper/src/index.ts`
- Modify: `js/pact-graphql-helper/src/types.ts`
- Create: `js/pact-graphql-helper/test/graphql-message.spec.ts`

**Step 1: Add failing tests**

Add tests for `graphqlMessageInteraction` that verify the message payload envelope and plugin contents.

**Step 2: Run tests**

Run: `npm test`
Expected: FAIL.

**Step 3: Implement helper**

Implement `graphqlMessageInteraction` using Pact JS async message API and the same canonical config builder.

**Step 4: Run tests**

Run: `npm test`
Expected: PASS.

**Step 5: Commit**

```bash
git add js/pact-graphql-helper/src/index.ts js/pact-graphql-helper/src/types.ts js/pact-graphql-helper/test/graphql-message.spec.ts
git commit -m "feat: add graphql message helper"
```

---

### Task 3: Update HTTP example to use new helper

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Update tests to use `graphqlHttpInteraction`**

Replace manual `withRequest` + `pluginContents` blocks with the new helper.

**Step 2: Run HTTP tests**

```bash
cd examples/js/product-consumer
npm test
```

Expected: PASS.

**Step 3: Commit**

```bash
git add examples/js/product-consumer/pact.test.ts
git commit -m "test: use graphql HTTP helper"
```

---

### Task 4: Add subscription message pact example

**Files:**
- Create: `examples/js/product-consumer/subscription-message.test.ts`
- Modify: `examples/js/product-consumer/package.json`

**Step 1: Add message pact test**

Create a test that uses `graphqlMessageInteraction` to generate a subscription event message pact.

**Step 2: Add npm script**

Add `"message": "vitest run subscription-message.test.ts"` to `package.json`.

**Step 3: Run message pact test**

```bash
npm run message
```

Expected: PASS and pact file written under `examples/js/product-consumer/pacts`.

**Step 4: Commit**

```bash
git add examples/js/product-consumer/subscription-message.test.ts examples/js/product-consumer/package.json
git commit -m "test: add subscription message pact example"
```

---

## Verification

- `npm test` in `js/pact-graphql-helper`
- `npm test` in `examples/js/product-consumer`
- `npm run message` in `examples/js/product-consumer`
