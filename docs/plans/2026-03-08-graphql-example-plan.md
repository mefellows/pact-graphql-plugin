# GraphQL Example Expansion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan.

**Goal:** Expand the JS example to demonstrate richer GraphQL schemas and a suite of positive and negative consumer tests.

**Architecture:** Keep a single SDL in `schema.graphql` and expand `pact.test.ts` with grouped tests that exercise schema validation and runtime mismatch behavior through the Pact plugin.

**Tech Stack:** Node.js (Vitest), Pact JS V4, pact-graphql-helper, GraphQL SDL

---

### Task 1: Add expanded schema SDL

**Files:**
- Modify: `examples/js/product-consumer/schema.graphql`
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Add failing tests for richer schema usage**

Add a new `describe('positive cases', ...)` with a test that queries nested fields, enums, and a valid mutation.

```ts
describe('positive cases', () => {
  it('supports nested product queries', async () => {
    // uses Product.variants.price.currency, Inventory.quantity, enums
  });
  it('supports placeOrder mutation', async () => {
    // uses OrderInput and nested response types
  });
  it('accepts subscription document validation', async () => {
    // uses inventoryChanged subscription document
  });
});
```

**Step 2: Run tests to confirm schema errors**

Run: `npm test`
Expected: FAIL with schema validation errors (missing types/fields/enums in SDL).

**Step 3: Expand schema SDL**

Replace `schema.graphql` with the e-commerce SDL that includes:

- Types: Product, Variant, Category, Inventory, Price, Money, Review, Customer, Order, OrderLine, Shipment, Address
- Enums: ProductStatus, CurrencyCode, OrderStatus, ShipmentStatus, ReviewRating
- Interface Node and union SearchResult
- Query, Mutation, Subscription roots

**Step 4: Run tests and confirm the positive cases pass**

Run: `npm test`
Expected: Positive tests pass; other tests may still be pending.

**Step 5: Commit**

```bash
git add examples/js/product-consumer/schema.graphql examples/js/product-consumer/pact.test.ts
git commit -m "feat: expand example schema"
```

### Task 2: Add schema-invalid query tests

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Add failing tests for schema-invalid queries**

```ts
describe('schema invalid', () => {
  it('rejects unknown field selections', async () => {
    // query Product.stockLevel (not in schema)
  });
  it('rejects invalid enum values', async () => {
    // query Product.status with DISCONTINUED if enum lacks it
  });
});
```

**Step 2: Run tests to verify expected failures**

Run: `npm test`
Expected: PASS (tests assert configuration-time rejection).

**Step 3: Commit**

```bash
git add examples/js/product-consumer/pact.test.ts
git commit -m "test: add schema-invalid query examples"
```

### Task 3: Add response-not-in-schema tests

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Add a failing test for unknown response fields**

```ts
describe('response not in schema', () => {
  it('rejects responses with unknown fields', async () => {
    // response includes Product.internalSku
  });
});
```

**Step 2: Run tests**

Run: `npm test`
Expected: PASS (tests assert configuration-time rejection).

**Step 3: Commit**

```bash
git add examples/js/product-consumer/pact.test.ts
git commit -m "test: add response schema validation examples"
```

### Task 4: Add runtime mismatch test

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Add a runtime mismatch test**

```ts
describe('runtime mismatch', () => {
  it('fails when runtime query omits canonical fields', async () => {
    // canonical query includes variants.price.currency
    // runtime payload omits variants block
  });
});
```

**Step 2: Run tests**

Run: `npm test`
Expected: PASS (test asserts the mismatch rejection).

**Step 3: Commit**

```bash
git add examples/js/product-consumer/pact.test.ts
git commit -m "test: add runtime mismatch example"
```

### Task 5: Stabilize example configuration

**Files:**
- Modify: `examples/js/product-consumer/pact.test.ts`

**Step 1: Pin plugin version for example runs**

Add near the top of the test file:

```ts
process.env.PACT_GRAPHQL_PLUGIN_VERSION ??= '0.1.0';
```

**Step 2: Run tests**

Run: `npm test`
Expected: PASS.

**Step 3: Commit**

```bash
git add examples/js/product-consumer/pact.test.ts
git commit -m "chore: pin example plugin version"
```

---

## Verification

- `npm test` in `examples/js/product-consumer`
- Optional: `cargo test -p pact_graphql_plugin` to ensure plugin stays green
