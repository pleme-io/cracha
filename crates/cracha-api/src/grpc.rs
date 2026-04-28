// gRPC Authorize service implementation.
//
// Pure read-side over the shared SharedIndex; no side effects.
// Sub-millisecond latency target — vigia is in the request hot
// path on every gated request.

use cracha_controller::SharedIndex;
use cracha_core::{AuthzRequest, Verb};
use cracha_proto::{
    AuthorizeRequest, AuthorizeResponse, Cracha, HealthRequest, HealthResponse,
};
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct CrachaService {
    pub index: SharedIndex,
}

#[tonic::async_trait]
impl Cracha for CrachaService {
    async fn authorize(
        &self,
        req: Request<AuthorizeRequest>,
    ) -> Result<Response<AuthorizeResponse>, Status> {
        let r = req.into_inner();

        let verb = parse_verb(&r.verb)
            .ok_or_else(|| Status::invalid_argument(format!("invalid verb: {}", r.verb)))?;

        let request = AuthzRequest {
            user: r.user,
            location: r.location,
            cluster: r.cluster,
            service: r.service,
            verb,
        };

        let idx = self.index.read().await;
        let decision = idx.authorize(&request);

        Ok(Response::new(AuthorizeResponse {
            allow: matches!(decision.decision, cracha_core::Decision::Allow),
            matched_policy: decision.matched_policy.unwrap_or_default(),
            reason: decision.reason,
        }))
    }

    async fn health(
        &self,
        _req: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let idx = self.index.read().await;
        Ok(Response::new(HealthResponse {
            healthy: true,
            policy_count: u32::try_from(idx.policy_count()).unwrap_or(u32::MAX),
        }))
    }
}

fn parse_verb(s: &str) -> Option<Verb> {
    match s.to_ascii_lowercase().as_str() {
        "read" => Some(Verb::Read),
        "write" => Some(Verb::Write),
        "delete" => Some(Verb::Delete),
        "admin" => Some(Verb::Admin),
        "*" | "all" => Some(Verb::All),
        _ => None,
    }
}
