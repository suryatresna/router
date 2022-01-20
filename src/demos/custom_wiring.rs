use http::HeaderValue;

use tower::util::BoxService;
use tower::{BoxError, ServiceBuilder, ServiceExt};
use tracing::info_span;

use crate::{
    PlannedRequest, Plugin, RouterRequest, RouterResponse, ServiceBuilderExt, SubgraphRequest,
};

#[derive(Default)]
struct MyPlugin;
impl Plugin for MyPlugin {
    fn subgraph_service(
        &mut self,
        _name: &str,
        service: BoxService<SubgraphRequest, RouterResponse, BoxError>,
    ) -> BoxService<SubgraphRequest, RouterResponse, BoxError> {
        ServiceBuilder::new()
            .instrument(|_| info_span!("subgraph_service"))
            .service(service)
            .boxed()
    }

    fn router_service(
        &mut self,
        service: BoxService<RouterRequest, RouterResponse, BoxError>,
    ) -> BoxService<RouterRequest, RouterResponse, BoxError> {
        ServiceBuilder::new()
            .instrument(|r: &RouterRequest| {
                info_span!(
                    "router_service",
                    correlation_id = r
                        .frontend_request
                        .headers()
                        .get("A")
                        .unwrap_or(&HeaderValue::from_static(""))
                        .to_str()
                        .unwrap()
                )
            })
            .service(service)
            .boxed()
    }

    fn query_planning_service(
        &mut self,
        service: BoxService<RouterRequest, PlannedRequest, BoxError>,
    ) -> BoxService<RouterRequest, PlannedRequest, BoxError> {
        ServiceBuilder::new()
            .instrument(|_| info_span!("query_planning_service"))
            .service(service)
            .boxed()
    }

    fn execution_service(
        &mut self,
        service: BoxService<PlannedRequest, RouterResponse, BoxError>,
    ) -> BoxService<PlannedRequest, RouterResponse, BoxError> {
        ServiceBuilder::new()
            .instrument(|_| info_span!("execution_service"))
            .service(service)
            .boxed()
    }
}
#[cfg(test)]
mod test {
    use crate::{
        graphql, ApolloRouter, ExecutionService, GraphQlSubgraphService, QueryPlannerService,
        RouterService,
    };
    use http::{Request, Uri};
    use std::str::FromStr;
    use tower::{BoxError, ServiceBuilder, ServiceExt};
    use tracing::{info, Level};

    #[tokio::test]
    async fn custom_wiring() -> Result<(), BoxError> {
        let _ = tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .try_init();

        //SubgraphService takes a SubgraphRequest and outputs a graphql::Response
        let book_service = ServiceBuilder::new()
            .service(
                GraphQlSubgraphService::builder()
                    .url(Uri::from_str("http://books").unwrap())
                    .build(),
            )
            .boxed_clone();

        //SubgraphService takes a SubgraphRequest and outputs a graphql::Response
        let author_service = ServiceBuilder::new()
            .service(
                GraphQlSubgraphService::builder()
                    .url(Uri::from_str("http://authors").unwrap())
                    .build(),
            )
            .boxed_clone();

        let query_planner_service = ServiceBuilder::new()
            .buffer(100) //My default implementations are not Clone
            .service(QueryPlannerService::default());

        let execution_service = ServiceBuilder::new()
            .buffer(100) //My default implementations are not Clone
            .service(
                ExecutionService::builder()
                    .subgraph_services(hashmap! {
                    "books".to_string()=> book_service,
                    "authors".to_string()=> author_service
                    })
                    .build(),
            );

        let service = RouterService::builder()
            .query_planner_service(query_planner_service)
            .query_execution_service(execution_service)
            .build();

        let router = ApolloRouter::from(service);

        let response = router
            .call(
                Request::builder()
                    .header("A", "HEADER_A")
                    .body(graphql::Request {
                        body: "Hello1".to_string(),
                    })
                    .unwrap(),
            )
            .await?;
        info!("{:?}", response);

        Ok(())
    }
}
