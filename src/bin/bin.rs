use tower::BoxError;
#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let router = apollo_router_rs::builder().build();
    router.start().await;
    Ok(())
}
