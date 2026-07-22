pub mod encoder;
pub mod graphql_payload;
pub mod interaction;
pub mod schema;
pub(crate) mod schema_index;
pub mod server;

pub use graphql_payload::{GraphqlInlineSchema, GraphqlRequestPayload};
pub use interaction::{GraphqlInteractionBuilder, GraphqlPluginConfig, GraphqlPluginRequest};
pub use schema::{SchemaRef, SchemaRegistry};
pub use server::GraphqlPlugin;

#[doc(hidden)]
pub mod test_support {
    //! Test-only seam. Not part of the supported API.
    pub use crate::schema_index::{SchemaIndex, TypeRef};

    pub fn schema_index_for(sdl: &str) -> anyhow::Result<SchemaIndex> {
        SchemaIndex::from_sdl(sdl)
    }
}
