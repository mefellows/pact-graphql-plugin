# GraphQL Pact Plugin Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust-based Pact plugin that normalizes GraphQL request bodies (JSON + query string), stores SDL metadata in pact files, and exposes a JS helper for easy consumer usage.

**Architecture:** `pact-graphql-plugin` crate hosts the plugin server plus helpers (`RequestEncoder`, `SchemaRegistry`, `GraphqlInteractionBuilder`). Plugin metadata is serialized into Pact interactions; schema blobs are hashed and persisted. A companion JS package wraps Pact consumer DSLs and forwards GraphQL configs to the plugin.

**Tech Stack:** Rust (pact_plugin_driver, serde, urlencoding), Node/TypeScript (ts-node/test runner), Pact plugin protocol.

---

### Task 1: Bootstrap Rust Workspace

**Files:**
- Create: `Cargo.toml`
- Create: `pact-graphql-plugin/Cargo.toml`
- Create: `pact-graphql-plugin/src/main.rs`
- Create: `pact-graphql-plugin/src/lib.rs`
- Create: `.gitignore`

**Step 1: Scaffold binary crate**

```bash
cargo new --bin pact-graphql-plugin --name pact_graphql_plugin --vcs none
```
Expected: new crate directory with `src/main.rs` and `Cargo.toml`.

**Step 2: Promote to workspace**

```bash
cat <<'EOF' > Cargo.toml
[workspace]
EOF
```

**Step 3: Add dependencies**

Edit `pact-graphql-plugin/Cargo.toml` to include the plugin driver using the alias form (crates.io publishes the hyphenated crate name, so we must map it back to the snake_case dependency key expected by Pact tooling):

```toml
[dependencies]
pact_plugin_driver = { version = "0.4", package = "pact-plugin-driver" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
urlencoding = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
tracing = "0.1"
anyhow = "1"
``` 

**Step 4: Add dev-dependencies for tests**

```toml
[dev-dependencies]
pretty_assertions = "1"
``` 

**Step 5: Wire binary entrypoint**

In `pact-graphql-plugin/src/main.rs`:

```rust
use pact_graphql_plugin::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::run().await
}
```

**Step 6: Run workspace check**

```bash
cargo check
```
Expected: PASS.

**Step 7: Commit**

```bash
git commit -m "chore: scaffold graphql plugin workspace"
```

### Task 2: Implement RequestEncoder (@superpowers/test-driven-development)

**Files:**
- Create: `pact-graphql-plugin/src/encoder.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`
- Create: `pact-graphql-plugin/tests/encoder_tests.rs`

**Step 1: Write failing tests**

```rust
// pact-graphql-plugin/tests/encoder_tests.rs
use pact_graphql_plugin::encoder::{GraphqlRequest, Transport, RequestEncoder};

#[test]
fn encodes_json_payload() {
    let req = GraphqlRequest::json("query { ping }", None, None);
    let encoded = RequestEncoder::encode(&req).unwrap();
    assert_eq!(encoded.body, r#"{\"query\":\"query { ping }\"}"#);
}

#[test]
fn encodes_query_string_payload() {
    let req = GraphqlRequest::query_string("query { pong }", None, None);
    let encoded = RequestEncoder::encode(&req).unwrap();
    assert_eq!(encoded.query_string.unwrap(), "query=query+%7B+pong+%7D");
}
```

**Step 2: Run failing tests**

```bash
cargo test encoder_tests
```
Expected: FAIL (module missing).

**Step 3: Implement encoder module**

```rust
// pact-graphql-plugin/src/encoder.rs
use serde_json::json;

pub enum Transport { JsonBody, QueryString }

pub struct GraphqlRequest<'a> {
    pub query: &'a str,
    pub operation_name: Option<&'a str>,
    pub variables_json: Option<&'a str>,
    pub transport: Transport,
}

pub struct EncodedRequest {
    pub body: Option<String>,
    pub query_string: Option<String>,
}

impl<'a> GraphqlRequest<'a> {
    pub fn json(query: &'a str, operation_name: Option<&'a str>, variables_json: Option<&'a str>) -> Self { /* ... */ }
    pub fn query_string(query: &'a str, operation_name: Option<&'a str>, variables_json: Option<&'a str>) -> Self { /* ... */ }
}

pub struct RequestEncoder;

impl RequestEncoder {
    pub fn encode(req: &GraphqlRequest) -> anyhow::Result<EncodedRequest> {
        match req.transport {
            Transport::JsonBody => { /* serialize JSON */ }
            Transport::QueryString => { /* url-encode */ }
        }
    }
}
```

Ensure helper uses `urlencoding::encode` and preserves field ordering.

**Step 4: Re-run tests**

```bash
cargo test encoder_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```bash
git commit -m "feat: add GraphQL request encoder"
```

### Task 3: Implement SchemaRegistry

**Files:**
- Create: `pact-graphql-plugin/src/schema.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`
- Create: `pact-graphql-plugin/tests/schema_tests.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn stores_and_retrieves_schema() {
    let dir = tempfile::tempdir().unwrap();
    let registry = SchemaRegistry::new(dir.path());
    let ref1 = registry.store("type Query { ping: String }").unwrap();
    assert!(ref1.hash.len() == 64);
    let inline = registry.inline_schema(&ref1.hash).unwrap();
    assert!(inline.contains("type Query"));
}
```

**Step 2: Run failing tests**

```bash
cargo test schema_tests
```

**Step 3: Implement registry**

```rust
pub struct SchemaRegistry {
    root: PathBuf,
}

impl SchemaRegistry {
    pub fn store(&self, sdl: &str) -> anyhow::Result<SchemaRef> {
        let canonical = sdl.trim().to_string() + "\n";
        let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        let path = self.root.join(format!("{}.graphql", hash));
        if !path.exists() {
            fs::write(&path, canonical)?;
        }
        Ok(SchemaRef { hash, encoding: "utf-8".into() })
    }

    pub fn inline_schema(&self, hash: &str) -> anyhow::Result<String> {
        let path = self.root.join(format!("{}.graphql", hash));
        Ok(fs::read_to_string(path)?)
    }
}
```

**Step 4: Tests**

```bash
cargo test schema_tests
```
Expected: PASS.

**Step 5: Commit**

```bash
git commit -m "feat: add schema registry"
```

### Task 4: GraphqlInteractionBuilder & Metadata Storage

**Files:**
- Create: `pact-graphql-plugin/src/interaction.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`
- Modify: `pact-graphql-plugin/src/encoder.rs` (reuse structs)
- Create: `pact-graphql-plugin/tests/interaction_tests.rs`

**Step 1: Define models**

```rust
#[derive(Serialize, Deserialize)]
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    pub transport: Transport,
    pub schema_ref: Option<SchemaRef>,
}
```

**Step 2: Write tests for validation**

```rust
#[test]
fn rejects_empty_query() {
    let err = builder.build(GraphqlPluginRequest { query_document: "".into(), ..Default::default() }).unwrap_err();
    assert!(err.to_string().contains("query"));
}
```

**Step 3: Implement builder**

```rust
impl GraphqlInteractionBuilder {
    pub fn build(&self, req: GraphqlPluginRequest) -> anyhow::Result<GraphqlPluginConfig> {
        if req.query_document.trim().is_empty() {
            bail!("query_document is required");
        }
        if let Some(schema) = req.schema_sdl {
            let reference = self.registry.store(&schema)?;
            // embed base64 inline copy
        }
        Ok(config)
    }
}
```

**Step 4: Tests**

```bash
cargo test interaction_tests
```

**Step 5: Commit**

```bash
git commit -m "feat: add GraphQL interaction builder"
```

### Task 5: Plugin Server Implementation

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs` (new)
- Modify: `pact-graphql-plugin/src/main.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`
- Create: `pact-graphql-plugin/tests/plugin_flow.rs`

**Step 1: Implement `server::run`**

```rust
pub async fn run() -> anyhow::Result<()> {
    let plugin = GraphqlPlugin::new();
    pact_plugin_driver::start_plugin_server("graphql", plugin).await
}
```

**Step 2: Implement trait methods**

```rust
#[async_trait]
impl PactPlugin for GraphqlPlugin {
    async fn configure_interaction(&self, req: ConfigureInteractionRequest) -> PluginResult<ConfigureInteractionResponse> {
        // deserialize plugin config json -> GraphqlPluginRequest
        // use GraphqlInteractionBuilder + RequestEncoder
    }

    async fn generate_content(...) { /* ensure stored payload returned */ }
}
```

**Step 3: Integration test using driver**

```rust
#[tokio::test]
async fn configure_and_generate_json_body() {
    let plugin = GraphqlPlugin::new_in_memory();
    let response = plugin.configure_interaction(req).await.unwrap();
    assert!(response.interaction.as_ref().unwrap().contents.body.is_some());
}
```

**Step 4: Run tests**

```bash
cargo test plugin_flow -- --nocapture
```

**Step 5: Commit**

```bash
git commit -m "feat: implement GraphQL pact plugin server"
```

### Task 6: JS Helper Package (@superpowers/test-driven-development)

**Files:**
- Create: `js/pact-graphql-helper/package.json`
- Create: `js/pact-graphql-helper/src/index.ts`
- Create: `js/pact-graphql-helper/src/types.ts`
- Create: `js/pact-graphql-helper/test/graphql-interaction.spec.ts`
- Create: `js/pact-graphql-helper/tsconfig.json`

**Step 1: Initialize package**

```bash
cd js && mkdir -p pact-graphql-helper && cd pact-graphql-helper
npm init -y
npm install typescript ts-node vitest pact @pact-foundation/pact-core
npx tsc --init --rootDir src --outDir dist --module commonjs --target ES2020
```

**Step 2: Write failing test**

```ts
import { describe, it, expect } from 'vitest';
import { graphqlInteraction } from '../src';

it('builds plugin config for JSON transport', async () => {
  const builder = fakeBuilder();
  await graphqlInteraction(builder, {
    schema: 'type Query { ping: String }',
    query: 'query { ping }',
    transport: 'json_body'
  });
  expect(builder.calledWith.plugin).toEqual('graphql');
});
```

**Step 3: Implement helper**

```ts
export async function graphqlInteraction(builder, options: GraphqlRequestOptions) {
  const pluginCfg = {
    query_document: dedent(options.query),
    operation_name: options.operationName,
    variables_json: JSON.stringify(options.variables ?? {}),
    transport: options.transport ?? 'json_body',
    schema_sdl: options.schema,
  };
  return builder.usingPlugin({ pluginName: 'graphql', configuration: pluginCfg });
}
```

**Step 4: Run tests**

```bash
cd js/pact-graphql-helper
npx vitest run
```

**Step 5: Commit**

```bash
git commit -m "feat: add JS graphql interaction helper"
```

### Task 7: Documentation & Example Pact

**Files:**
- Create: `examples/js/product-consumer/pact.test.ts`
- Modify: `README.md`
- Modify: `docs/plans/2026-03-04-graphql-plugin-design.md` (link to implementation)

**Step 1: Add example consumer test**

```ts
import { pactWith } from '@pact-foundation/pact';
import { graphqlInteraction } from '../../js/pact-graphql-helper';

pactWith({ consumer: 'product-consumer', provider: 'product-provider' }, (interaction) => {
  interaction.addInteraction(async (builder) => {
    await graphqlInteraction(builder, {
      schema: fs.readFileSync('schema.graphql', 'utf8'),
      query: `query GetProduct($id: ID!) { product(id: $id) { id name type } }`,
      variables: { id: '10' }
    });
    builder.willRespondWith({ status: 200, body: { data: { product: { id: '10' } } } });
  });
});
```

**Step 2: Document usage**

Update `README.md` with installation, SDL embedding, JSON vs query-string guidance, and instructions for enabling plugin in Pact tests.

**Step 3: Run example test**

```bash
cd examples/js/product-consumer
npm install
npx vitest run
```
Expected: PASS, pact file generated with `plugin_config.graphql` block.

**Step 4: Commit**

```bash
git commit -m "docs: add graphql plugin usage example"
```

---

Plan complete and saved to `docs/plans/2026-03-04-graphql-plugin-plan.md`. Two execution options:

1. **Subagent-Driven (this session)** – I dispatch a fresh subagent per task with @superpowers/subagent-driven-development for tight feedback.
2. **Parallel Session (separate)** – Start a new session dedicated to implementation using @superpowers/executing-plans.

Which approach would you like to use? 
