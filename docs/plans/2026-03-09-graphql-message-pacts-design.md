# GraphQL Message Pacts Design

## Goal

Add message-pact support for GraphQL subscription events, plus a helper API that removes duplication in the HTTP DSL. Provide an example that generates a subscription message pact using the helper.

## Scope

### Helper API

1) `graphqlHttpInteraction(interaction, { schema, query, variables, operationName, transport? })`

- Wraps the Pact V4 HTTP interaction and preconfigures:
  - `withRequest('POST', '/graphql', ...)`
  - `content-type` header
  - `pluginContents` with canonical GraphQL payload
- Removes manual `pluginContents` duplication from examples.

2) `graphqlMessageInteraction(pact, { schema, subscription, variables, data, operationName })`

- Creates an async message interaction for a subscription event (operationName required so the envelope is stable).
- Payload envelope:

```json
{
  "subscription": "InventoryChanged",
  "variables": { "variantId": "var-1" },
  "data": { "inventoryChanged": { "quantity": 42, "updatedAt": "2026-03-08T12:00:00Z" } }
}
```

### Examples

- Update `examples/js/product-consumer/pact.test.ts` to use `graphqlHttpInteraction` (no manual `pluginContents`).
- Add `examples/js/product-consumer/subscription-message.test.ts` to generate a message pact using `graphqlMessageInteraction`.
- Add npm script `message` to run the message pact test separately.

## Success Criteria

- HTTP examples no longer duplicate GraphQL config in `pluginContents`.
- Message pact example generates a pact file in `examples/js/product-consumer/pacts`.
- `npm test` (HTTP) and `npm run message` (message pact) both pass.
