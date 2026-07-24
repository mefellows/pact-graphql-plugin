pub mod encoder;
pub mod graphql_payload;
pub mod interaction;
pub(crate) mod query_ast;
// TODO(task-8): remove this allow once `validate_response` is wired into the
// plugin's comparison pipeline (the response content-type support promised by
// the plan's Architecture section). Until then this module has no production
// caller — its unit tests are its only consumer under `cargo build`.
#[allow(dead_code)]
pub(crate) mod response;
pub mod schema;
pub(crate) mod schema_index;
pub mod server;

pub use graphql_payload::{GraphqlInlineSchema, GraphqlRequestPayload};
pub use interaction::{GraphqlInteractionBuilder, GraphqlPluginConfig, GraphqlPluginRequest};
pub use schema::{SchemaRef, SchemaRegistry};
pub use server::GraphqlPlugin;
