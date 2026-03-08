# GraphQL Example Expansion Design

## Goal

Expand `examples/js/product-consumer` into a richer GraphQL example that demonstrates:

- Positive (schema-valid) consumer interactions.
- Schema-invalid queries.
- Consumer expectations that reference schema-unknown response fields.
- Runtime mismatches where the executed query differs from the canonical payload.

The example remains HTTP-only (POST /graphql). Subscriptions are schema-validated only.

## Scope

### Schema

E-commerce catalog and checkout domain with medium complexity:

- Types: Product, Variant, Category, Inventory, Price, Money, Review, Customer, Order, OrderLine, Shipment, Address
- Enums: ProductStatus, CurrencyCode, OrderStatus, ShipmentStatus, ReviewRating
- Interface: Node (id)
- Union: SearchResult (Product | Category | Review)
- Queries: product, products, search, order
- Mutations: addToCart, placeOrder, submitReview
- Subscriptions: inventoryChanged, orderStatusChanged

### Tests

Organize tests under `examples/js/product-consumer/pact.test.ts` into describe blocks:

1. Positive cases
   - Valid query with nested selections and enums.
   - Valid mutation with nested input.
   - Valid subscription document (schema validation only).

2. Schema-invalid query
   - Query references a field that does not exist.
   - Query references an invalid enum literal.

3. Response not in schema
   - Response contains fields not defined on the schema.

4. Runtime mismatch
   - Canonical query includes a nested selection; runtime query omits it.

## Implementation Notes

- Keep a single schema file: `examples/js/product-consumer/schema.graphql`.
- Maintain a single Pact for the example; use unique interaction names per test.
- Subscriptions are not executed; validation is via SDL only.
- Ensure tests pin `PACT_GRAPHQL_PLUGIN_VERSION` in the example setup to avoid ambiguity.

## Success Criteria

- Positive cases pass.
- Schema-invalid and response-not-in-schema cases fail at configuration time with clear errors.
- Runtime mismatch yields a `GraphQL query document differs` mismatch from the plugin matcher.
