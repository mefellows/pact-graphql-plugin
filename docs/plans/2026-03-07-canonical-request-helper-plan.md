# Canonical Request Helper Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract canonical GraphQL request helpers into `graphql_payload.rs` so configure-time and runtime flows share constructors, validation, and diffing.

**Architecture:** Keep canonical helpers next to `GraphqlRequestPayload`, introduce `CanonicalGraphqlRequest` with shared helper fns, and update `interaction.rs` to delegate to the new API without changing external behavior.

**Tech Stack:** Rust, serde, anyhow, graphql_parser, base64, SchemaRegistry utility.

---

### Task 1: Add Canonical Request Types & Helpers

**Files:**
- Modify: `pact-graphql-plugin/src/graphql_payload.rs`

**Step 1: Define structs**

Add `CanonicalGraphqlRequest` and `RequestMismatch` structs adjacent to existing payload types, deriving `Clone + Debug`. Keep JSON pointer note in doc comment.

**Step 2: Move helper functions**

Cut `canonicalize_query`, `canonicalize_variables`, `canonicalize_sdl`, newline helpers, and GraphQL validation utilities (`SchemaIndex`, validation functions) from `interaction.rs` into this file unchanged. Ensure visibility (`pub(crate)`) fits existing callers.

**Step 3: Wire internal helpers**

Refactor moved functions to use shared imports (serde_json, graphql_parser, etc.) by updating module-level `use` lines.

### Task 2: Implement Constructors on CanonicalGraphqlRequest

**Files:**
- Modify: `pact-graphql-plugin/src/graphql_payload.rs`
- Modify: `pact-graphql-plugin/src/schema.rs` (read-only for registry API reference)

**Step 1: from_interaction_config**

Wrap existing `GraphqlInteractionBuilder::build` canonicalization logic inside `CanonicalGraphqlRequest::from_interaction_config`. Ensure it stores schemas via `SchemaRegistry` and returns inline schema metadata.

**Step 2: from_http_request**

Implement HTTP constructor that: parses body per `Transport`, handles query-string JSON fallback, canonicalizes fields, decodes optional `schema_base64`, fetches via registry when missing, validates using shared helpers, and returns canonical payload & inline schema.

### Task 3: Add Diff Helper

**Files:**
- Modify: `pact-graphql-plugin/src/graphql_payload.rs`

**Step 1: Implement `diff`**

Compare `payload` fields and inline schema base64 strings. For inequality, push `RequestMismatch` entries with JSON pointer `path`, stringified values, and human-readable `description`. Ensure deterministic ordering (compare fields in fixed sequence).

### Task 4: Update Interaction Builder to Use Canonical Helper

**Files:**
- Modify: `pact-graphql-plugin/src/interaction.rs`

**Step 1: Import helper**

Replace existing helper functions with a `use crate::graphql_payload::CanonicalGraphqlRequest;` (plus others as needed).

**Step 2: Delegate build logic**

Within `GraphqlInteractionBuilder::build`, call `CanonicalGraphqlRequest::from_interaction_config(req, &self.registry)?`, then populate `GraphqlPluginConfig` using the returned canonical payload & inline schema info. Remove now-unneeded helper definitions from this file.

### Task 5: Format & Check

**Files:**
- N/A

**Step 1: Run formatter**

```bash
cargo fmt
```

Expect no diffs beyond intentional changes.

**Step 2: Build check**

```bash
cargo check
```

Ensure compilation succeeds without warnings. Address any errors before completion.
