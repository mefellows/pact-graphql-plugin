# Request Encoder Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a reusable GraphQL request encoder that supports JSON bodies and query strings in the `pact-graphql-plugin` crate.

**Architecture:** Introduce an `encoder` module exposing owned `GraphqlRequest` models, a `Transport` enum, and a stateless `RequestEncoder` that converts inputs into either serialized JSON or URL-encoded strings. Follows TDD with integration tests in `tests/encoder_tests.rs`.

**Tech Stack:** Rust, `serde_json`, `urlencoding`, `anyhow`, `cargo test`, `cargo fmt`.

---

### Task 1: Add failing encoder tests (@superpowers/test-driven-development)

**Files:**
- Create: `pact-graphql-plugin/tests/encoder_tests.rs`

**Step 1: Write the failing test**

```rust
use pact_graphql_plugin::encoder::{GraphqlRequest, Transport, RequestEncoder};

#[test]
fn encodes_json_payload() {
    let req = GraphqlRequest::json("query { ping }", None, None);
    let encoded = RequestEncoder::encode(&req).unwrap();
    assert_eq!(encoded.body.as_deref(), Some("{\"query\":\"query { ping }\"}"));
    assert!(encoded.query_string.is_none());
}

#[test]
fn encodes_query_string_payload() {
    let req = GraphqlRequest::query_string("query { pong }", None, None);
    let encoded = RequestEncoder::encode(&req).unwrap();
    assert_eq!(encoded.query_string.as_deref(), Some("query=query+%7B+pong+%7D"));
    assert!(encoded.body.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test encoder_tests`

Expected: FAIL with missing module/struct errors.

### Task 2: Implement encoder module (@superpowers/test-driven-development)

**Files:**
- Create: `pact-graphql-plugin/src/encoder.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`

**Step 1: Implement minimal encoder types**

```rust
use serde_json::json;
use urlencoding::encode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transport {
    JsonBody,
    QueryString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphqlRequest {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    pub transport: Transport,
}

impl GraphqlRequest {
    pub fn json(query: impl Into<String>, operation_name: Option<impl Into<String>>, variables_json: Option<impl Into<String>>) -> Self { /* populate struct */ }
    pub fn query_string(query: impl Into<String>, operation_name: Option<impl Into<String>>, variables_json: Option<impl Into<String>>) -> Self { /* populate struct */ }
}

pub struct EncodedRequest {
    pub body: Option<String>,
    pub query_string: Option<String>,
}

pub struct RequestEncoder;

impl RequestEncoder {
    pub fn encode(req: &GraphqlRequest) -> anyhow::Result<EncodedRequest> {
        match req.transport {
            Transport::JsonBody => { /* serialize JSON */ }
            Transport::QueryString => { /* URL encode */ }
        }
    }
}
```

**Step 2: Re-export module in lib**

Add `pub mod encoder;` to `pact-graphql-plugin/src/lib.rs`.

**Step 3: Run formatter**

Run: `cargo fmt`

**Step 4: Run tests to ensure they pass**

Run: `cargo test encoder_tests -- --nocapture`

Expected: PASS, prints encoded payloads.

### Task 3: Commit changes

**Files:**
- `pact-graphql-plugin/src/encoder.rs`
- `pact-graphql-plugin/src/lib.rs`
- `pact-graphql-plugin/tests/encoder_tests.rs`

**Step 1: Stage files**

```bash
git add pact-graphql-plugin/src/encoder.rs pact-graphql-plugin/src/lib.rs pact-graphql-plugin/tests/encoder_tests.rs
```

**Step 2: Commit**

```bash
git commit -m "feat: add GraphQL request encoder"
```

---

Plan complete and saved to `docs/plans/2026-03-04-request-encoder-implementation-plan.md`. Two execution options:

1. **Subagent-Driven (this session)** – dispatch fresh subagent per task using @superpowers/subagent-driven-development for fast iteration.
2. **Parallel Session (separate)** – open a new session using @superpowers/executing-plans to implement in another workspace.

Which approach would you like to use?
