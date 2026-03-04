pub mod encoder;
pub mod interaction;
pub mod schema;

pub use interaction::{GraphqlInteractionBuilder, GraphqlPluginConfig, GraphqlPluginRequest};
pub use schema::{SchemaRef, SchemaRegistry};

pub mod server {
    pub async fn run() -> anyhow::Result<()> {
        Ok(())
    }
}
