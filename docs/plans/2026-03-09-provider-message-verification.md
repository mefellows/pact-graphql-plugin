# Provider Message Verification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make provider message verification use real provider logic instead of a hardcoded payload, while keeping the current vitest provider verification flow.

**Architecture:** Introduce a provider-side helper that builds the inventory subscription payload from in-memory data. Use it in the live resolver path and in `messageProviders` so the verification exercises actual provider code.

**Tech Stack:** TypeScript, Apollo Server, @pact-foundation/pact, Vitest.

---

### Task 1: Add provider helper for inventory message payload

**Files:**
- Modify: `examples/js/product-consumer/provider-server.ts`

**Step 1: Write the failing test**

Add a tiny unit-style assertion to the provider verification test to ensure the helper is used (placeholder, will fail until helper exists):

```ts
import { buildInventoryChangedEvent } from './provider-server';

it('builds inventory message payload from provider logic', () => {
  const event = buildInventoryChangedEvent('var-1');
  expect(event).toEqual({
    subscription: 'InventoryChanged',
    variables: { variantId: 'var-1' },
    data: {
      inventoryChanged: { quantity: 42, updatedAt: '2026-03-08T12:00:00Z' },
    },
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run verify-provider.test.ts`

Expected: FAIL with `buildInventoryChangedEvent` not found/exported.

**Step 3: Write minimal implementation**

In `provider-server.ts`, add and export a helper that finds the variant and returns the message payload:

```ts
export function buildInventoryChangedEvent(variantId: string) {
  const variant = getVariant(variantId);
  if (!variant) {
    throw new Error('Variant not found');
  }
  return {
    subscription: 'InventoryChanged',
    variables: { variantId },
    data: { inventoryChanged: variant.inventory },
  };
}
```

Wire the helper into `Variant.inventory` so it is used by the live API path:

```ts
Variant: {
  inventory: (variant: { inventory: unknown; id: string }) =>
    buildInventoryChangedEvent(variant.id).data.inventoryChanged,
},
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run verify-provider.test.ts`

Expected: PASS

**Step 5: Commit**

```bash
  examples/js/product-consumer/verify-provider.test.ts
```

### Task 2: Replace hardcoded message provider payload with helper

**Files:**
- Modify: `examples/js/product-consumer/verify-provider.test.ts`

**Step 1: Write the failing test**

No new test needed; update the message provider to call the helper.

**Step 2: Run test to verify it fails**

Skip (refactor).

**Step 3: Write minimal implementation**

Replace the literal object in `messageProviders` with:

```ts
import { buildInventoryChangedEvent } from './provider-server';

messageProviders: {
  'a GraphQL subscription event': async () => buildInventoryChangedEvent('var-1'),
},
```

**Step 4: Run test to verify it passes**

Run: `npm test`

Expected: PASS

**Step 5: Commit**

```bash
```

### Task 3: Regenerate pacts and verify provider end-to-end

**Files:**
- Test: `examples/js/product-consumer/pact.test.ts`
- Test: `examples/js/product-consumer/subscription-message.test.ts`

**Step 1: Regenerate pacts**

Run: `npx vitest run pact.test.ts subscription-message.test.ts`

Expected: PASS

**Step 2: Verify provider**

Run: `npx vitest run verify-provider.test.ts`

Expected: PASS

**Step 3: Commit (if new fixes required)**

Only if additional fixes were required:

```bash
git commit -m "test: stabilize provider message verification"
```
