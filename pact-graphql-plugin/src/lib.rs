pub mod encoder;
pub mod schema;

pub use schema::{SchemaRef, SchemaRegistry};

pub mod server {
    pub async fn run() -> anyhow::Result<()> {
        Ok(())
    }
}
