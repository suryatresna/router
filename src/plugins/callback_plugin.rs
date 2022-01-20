use crate::{PlannedRequest, Plugin, RouterRequest, RouterResponse, SubgraphRequest};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tower::util::BoxService;
use tower::{BoxError, ServiceBuilder, ServiceExt};

pub fn builder() -> CallbackPluginBuilder {
    CallbackPluginBuilder::default()
}

pub trait CallbackPlugin {
    fn before_router(&self, router_request: RouterRequest) -> RouterRequest {
        router_request
    }
    fn after_router(&self, router_response: RouterResponse) -> RouterResponse {
        router_response
    }

    fn before_query_planning(&self, router_request: RouterRequest) -> RouterRequest {
        router_request
    }

    fn after_query_planning(&self, planned_request: PlannedRequest) -> PlannedRequest {
        planned_request
    }

    fn before_execution(&self, planned_request: PlannedRequest) -> PlannedRequest {
        planned_request
    }

    fn after_execution(&self, router_response: RouterResponse) -> RouterResponse {
        router_response
    }

    fn before_subgraph(&self, _name: &str, subgraph_request: SubgraphRequest) -> SubgraphRequest {
        subgraph_request
    }
    fn after_subgraph(&self, _name: &str, router_response: RouterResponse) -> RouterResponse {
        router_response
    }
}

impl<CallbackPluginImplementation> Plugin for CallbackPluginImplementation
where
    CallbackPluginImplementation: CallbackPlugin + Send + Sync + Clone + 'static,
{
    fn router_service(
        &mut self,
        service: BoxService<RouterRequest, RouterResponse, BoxError>,
    ) -> BoxService<RouterRequest, RouterResponse, BoxError> {
        let clone_for_before = self.clone();
        let clone_for_after = self.clone();
        ServiceBuilder::new()
            .map_request(move |request| clone_for_before.before_router(request))
            .map_response(move |response| clone_for_after.after_router(response))
            .service(service)
            .boxed()
    }

    fn query_planning_service(
        &mut self,
        service: BoxService<RouterRequest, PlannedRequest, BoxError>,
    ) -> BoxService<RouterRequest, PlannedRequest, BoxError> {
        let clone_for_before = self.clone();
        let clone_for_after = self.clone();
        ServiceBuilder::new()
            .map_request(move |request| clone_for_before.before_query_planning(request))
            .map_response(move |planned_request| {
                clone_for_after.after_query_planning(planned_request)
            })
            .service(service)
            .boxed()
    }

    fn execution_service(
        &mut self,
        service: BoxService<PlannedRequest, RouterResponse, BoxError>,
    ) -> BoxService<PlannedRequest, RouterResponse, BoxError> {
        let clone_for_before = self.clone();
        let clone_for_after = self.clone();
        ServiceBuilder::new()
            .map_request(move |planned_request| clone_for_before.before_execution(planned_request))
            .map_response(move |router_response| clone_for_after.after_execution(router_response))
            .service(service)
            .boxed()
    }

    fn subgraph_service(
        &mut self,
        name: &str,
        service: BoxService<SubgraphRequest, RouterResponse, BoxError>,
    ) -> BoxService<SubgraphRequest, RouterResponse, BoxError> {
        let name_for_before = Cow::from(name.to_string());
        let name_for_after = name_for_before.clone();
        let clone_for_before = self.clone();
        let clone_for_after = self.clone();

        ServiceBuilder::new()
            .map_request(move |subgraph_request| {
                clone_for_before.before_subgraph(&name_for_before, subgraph_request)
            })
            .map_response(move |router_response| {
                clone_for_after.after_subgraph(&name_for_after, router_response)
            })
            .service(service)
            .boxed()
    }
}

#[derive(Default, Clone)]
pub struct CallbackPluginBuilder {
    before_router: Option<Arc<dyn Fn(RouterRequest) -> RouterRequest + Send + Sync + 'static>>,
    after_router: Option<Arc<dyn Fn(RouterResponse) -> RouterResponse + Send + Sync + 'static>>,

    before_query_planning:
        Option<Arc<dyn Fn(RouterRequest) -> RouterRequest + Send + Sync + 'static>>,
    after_query_planning:
        Option<Arc<dyn Fn(PlannedRequest) -> PlannedRequest + Send + Sync + 'static>>,

    before_execution: Option<Arc<dyn Fn(PlannedRequest) -> PlannedRequest + Send + Sync + 'static>>,
    after_execution: Option<Arc<dyn Fn(RouterResponse) -> RouterResponse + Send + Sync + 'static>>,

    before_any_subgraph:
        Vec<Arc<dyn Fn(SubgraphRequest) -> SubgraphRequest + Send + Sync + 'static>>,
    after_any_subgraph: Vec<Arc<dyn Fn(RouterResponse) -> RouterResponse + Send + Sync + 'static>>,

    before_subgraph:
        HashMap<String, Arc<dyn Fn(SubgraphRequest) -> SubgraphRequest + Send + Sync + 'static>>,
    after_subgraph:
        HashMap<String, Arc<dyn Fn(RouterResponse) -> RouterResponse + Send + Sync + 'static>>,
}

macro_rules! with {
    ($name:ident, $fn_type:ty) => {
        paste::item! {
            pub fn [< with _ $name >](self, $name: impl $fn_type + Send + Sync + 'static) -> Self {
                if self.$name.is_some() {
                    panic!("[< with _ $name >] cannot be invoked twice, please build an other one");
                }

                Self {
                    $name: Some(Arc::new($name)),
                    ..self
                }
            }
        }
    };
}

impl CallbackPluginBuilder {
    pub fn build(self) -> Self {
        self
    }

    with!(before_router, Fn(RouterRequest) -> RouterRequest);
    with!(after_router, Fn(RouterResponse) -> RouterResponse);

    with!(before_query_planning, Fn(RouterRequest) -> RouterRequest);
    with!(after_query_planning,Fn(PlannedRequest) -> PlannedRequest);

    with!(before_execution,Fn(PlannedRequest) -> PlannedRequest);
    with!(after_execution, Fn(RouterResponse) -> RouterResponse);

    pub fn with_before_any_subgraph(
        mut self,
        callback: impl Fn(SubgraphRequest) -> SubgraphRequest + Send + Sync + 'static,
    ) -> Self {
        self.before_any_subgraph.push(Arc::new(callback));

        self
    }

    pub fn with_after_any_subgraph(
        mut self,
        callback: impl Fn(RouterResponse) -> RouterResponse + Send + Sync + 'static,
    ) -> Self {
        self.after_any_subgraph.push(Arc::new(callback));

        self
    }

    pub fn with_before_subgraph(
        mut self,
        service_name: String,
        callback: impl Fn(SubgraphRequest) -> SubgraphRequest + Send + Sync + 'static,
    ) -> Self {
        if self.before_subgraph.contains_key(service_name.as_str()) {
            panic!("with_before_subgraph cannot be invoked twice on the same service_name, please build an other one");
        }

        self.before_subgraph
            .insert(service_name, Arc::new(callback));

        Self { ..self }
    }

    pub fn with_after_subgraph(
        mut self,
        service_name: String,
        callback: impl Fn(RouterResponse) -> RouterResponse + Send + Sync + 'static,
    ) -> Self {
        if self.after_subgraph.contains_key(service_name.as_str()) {
            panic!("with_before_subgraph cannot be invoked twice on the same service_name, please build an other one");
        }

        self.after_subgraph.insert(service_name, Arc::new(callback));

        Self { ..self }
    }
}

impl CallbackPlugin for CallbackPluginBuilder {
    fn before_router(&self, router_request: RouterRequest) -> RouterRequest {
        if let Some(before_router) = &self.before_router {
            before_router(router_request)
        } else {
            router_request
        }
    }
    fn after_router(&self, router_response: RouterResponse) -> RouterResponse {
        if let Some(after_router) = &self.after_router {
            after_router(router_response)
        } else {
            router_response
        }
    }

    fn before_query_planning(&self, router_request: RouterRequest) -> RouterRequest {
        if let Some(before_query_planning) = &self.before_query_planning {
            before_query_planning(router_request)
        } else {
            router_request
        }
    }

    fn after_query_planning(&self, planned_request: PlannedRequest) -> PlannedRequest {
        if let Some(after_query_planning) = &self.after_query_planning {
            after_query_planning(planned_request)
        } else {
            planned_request
        }
    }

    fn before_execution(&self, planned_request: PlannedRequest) -> PlannedRequest {
        if let Some(before_execution) = &self.before_execution {
            before_execution(planned_request)
        } else {
            planned_request
        }
    }

    fn after_execution(&self, router_response: RouterResponse) -> RouterResponse {
        if let Some(after_execution) = &self.after_execution {
            after_execution(router_response)
        } else {
            router_response
        }
    }

    fn before_subgraph(&self, name: &str, subgraph_request: SubgraphRequest) -> SubgraphRequest {
        // run before any hooks
        let subgraph_request = self
            .before_any_subgraph
            .iter()
            .fold(subgraph_request, |request, callback| callback(request));
        // run specific hook
        if let Some(before_subgraph) = self.before_subgraph.get(name) {
            before_subgraph(subgraph_request)
        } else {
            subgraph_request
        }
    }

    fn after_subgraph(&self, name: &str, router_response: RouterResponse) -> RouterResponse {
        // run specific hook
        let router_response = if let Some(after_subgraph) = &self.after_subgraph.get(name) {
            after_subgraph(router_response)
        } else {
            router_response
        };
        // run after any hooks
        self.after_any_subgraph
            .iter()
            .fold(router_response, |response, callback| callback(response))
    }
}
