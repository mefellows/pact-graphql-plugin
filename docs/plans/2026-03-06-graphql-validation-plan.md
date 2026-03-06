# GraphQL Validation & Helper Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enforce GraphQL request/schema validation in the plugin and expose a higher-level JS helper that wires Pact interactions automatically while embedding canonical SDL in pact files.

**Architecture:** Plugin stores canonical GraphQL request metadata + inline SDL during `configure_interaction`, validates documents against the SDL, and compares incoming requests via `compare_contents`. The JS helper wraps Pact V4 interactions, calling the plugin and executing tests without manual request/response wiring.

**Tech Stack:** Rust (graphql-parser, serde, pact_plugin_driver), TypeScript (Vitest, Pact JS), just (existing workflows).

---

### Task 1: Enrich plugin interaction config

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`
- Modify: `pact-graphql-plugin/src/encoder.rs` (if helpers needed)
- Modify: `pact-graphql-plugin/src/server.rs`
- Tests: `pact-graphql-plugin/tests/interaction_tests.rs`

**Steps:**
1. Update `GraphqlPluginConfig` to include canonical request fields (`request_payload`) and `schema_inline_base64`.
2. Extend `GraphqlInteractionBuilder::build` to compute Base64 SDL, store request payload, and validate GraphQL document against SDL using a Rust GraphQL parser crate. Add unit tests covering success/failure paths.
3. Ensure serialization/deserialization includes the new fields; update `config_to_struct` + round-trip tests.
4. Commit `feat: enrich graphql interaction config`.

### Task 2: Implement request comparison

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs`
- Tests: `pact-graphql-plugin/tests/plugin_flow.rs` (extend) or new integration test

**Steps:**
1. Implement helper to parse incoming HTTP body (JSON vs query string) into normalized structure (sorted fields, trimmed doc).
2. Update `compare_contents` to load expected payload from `GraphqlPluginConfig` and compare with actual; return mismatch entries when they differ. Add integration test that submits mismatched query and asserts failure message.
3. Commit `feat: validate graphql request payloads`.

### Task 3: Modernize JS helper API

**Files:**
- Modify: `js/pact-graphql-helper/src/index.ts`
- Modify: `js/pact-graphql-helper/src/types.ts`
- Modify: `js/pact-graphql-helper/test/graphql-interaction.spec.ts`
- Modify: `examples/js/product-consumer/pact.test.ts`

**Steps:**
1. Redesign `graphqlInteraction` to accept high-level options (`pact`, `description`, `schema`, `query`, `variables`, `operationName`, `given`, `expected`). It should internally call `pact.addInteraction()`, configure request/response, and return `executeTest` promise.
2. Update helper implementation to dedupe query normalization, apply plugin request metadata, and set response JSON automatically. Support optional headers/status + matcher passthrough.
3. Revise unit tests + example Pact test to use the simplified helper signature.
4. Commit `feat: simplify graphql helper API`.

### Task 4: Schema validation integration tests

**Files:**
- Tests: `pact-graphql-plugin/tests/schema_validation_tests.rs` (new) + JS example test updates (optional)

**Steps:**
1. Add Rust integration test verifying invalid query fails against SDL.
2. Extend JS example or README to demonstrate drift failure (expect helper call to reject).
3. Commit `test: add graphql schema validation coverage`.

### Task 5: Docs + README updates

**Files:**
- Modify: `README.md`
- Modify: `docs/plans/2026-03-06-graphql-validation-design.md` (tie design to implementation)

**Steps:**
1. Document new helper API and pact metadata (inline SDL + enforced request matching).
2. Add troubleshooting note for schema mismatch errors.
3. Commit `docs: describe graphql validation workflow`.

### Task 6: Bundle/install smoke test

**Files:**
- n/a

**Steps:**
1. Run `just bundle x86_64-apple-darwin && just install`.
2. Run `cd examples/js/product-consumer && npm test` to confirm full flow.
3. Report results (no commit unless artifacts change).

---

Plan complete and saved to `docs/plans/2026-03-06-graphql-validation-plan.md`. Two execution options:

1. Subagent-Driven (this session)
2. Parallel Session (separate, using executing-plans)

Which approach should we use?
