use pact_graphql_plugin::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::run().await
}
