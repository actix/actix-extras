use crate::storage::SessionStore;
use crate::Session;
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

struct SessionMiddleware<Store: SessionStore> {
    pub(crate) storage_backend: Arc<Store>,
}

impl<S, B, Store> Transform<S, ServiceRequest> for SessionMiddleware<Store>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
    Store: SessionStore + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = InnerSessionMiddleware<S, Store>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(InnerSessionMiddleware {
            service: Rc::new(service),
            storage_backend: self.storage_backend.clone(),
        }))
    }
}

#[non_exhaustive]
#[doc(hidden)]
pub struct InnerSessionMiddleware<S, Store: SessionStore + 'static> {
    service: Rc<S>,
    storage_backend: Arc<Store>,
}

#[allow(clippy::type_complexity)]
impl<S, B, Store> Service<ServiceRequest> for InnerSessionMiddleware<S, Store>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
    Store: SessionStore + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let storage_backend = self.storage_backend.clone();

        Box::pin(async move {
            let (request, payload) = req.into_parts();
            let session = storage_backend.load(&request).unwrap();
            let mut req = ServiceRequest::from_parts(request, payload);
            let mut metadata = None;
            if let Some((m, state)) = session {
                Session::set_session(&mut req, state);
                metadata = Some(m);
            }
            let mut res = service.call(req).await?;
            let (_status, state) = Session::get_changes(&mut res);
            let state: HashMap<String, String> = state.collect();
            storage_backend
                .save(res.response_mut().head_mut(), (metadata, state))
                .unwrap();
            Ok(res)
        })
    }
}
