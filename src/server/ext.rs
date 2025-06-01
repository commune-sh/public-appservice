use std::future::Future;

use axum::{
    async_trait,
    body::Body,
    extract::{FromRequest, FromRequestParts, Request},
    response::{IntoResponse, Response},
    routing::{on, MethodFilter},
    Router,
};

use bytes::BytesMut;
use http::Method;
use ruma::api::{IncomingRequest, Metadata, OutgoingResponse};

pub struct Incoming<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for Incoming<T>
where
    T: IncomingRequest,
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request(_req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

#[derive(Clone)]
pub struct Outgoing<T>(pub T);

impl<T> IntoResponse for Outgoing<T>
where
    T: OutgoingResponse,
{
    fn into_response(self) -> Response {
        match self.0.try_into_http_response::<BytesMut>() {
            Ok(res) => res.map(BytesMut::freeze).map(Body::from).into_response(),
            Err(_) => todo!(),
        }
    }
}

#[allow(dead_code)]
trait RouterExt<S> {
    fn ruma_route<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static;
}

impl<S> RouterExt<S> for Router<S> {
    fn ruma_route<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        handler.add_to_router(self)
    }
}

#[allow(dead_code)]
pub trait Handler<T, S> {
    fn add_to_router(self, router: Router<S>) -> Router<S>;
}

macro_rules! def_method_to_filter {
    ($($variant:ident),* $(,)?) => {
        fn method_to_filter(method: Method) -> MethodFilter {
            match method {
                $(Method::$variant => MethodFilter::$variant,)*
                m => panic!("Unsupported HTTP method: {m:?}"),
            }
        }
    };
}

def_method_to_filter!(DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT, TRACE);

#[allow(non_snake_case)]
#[async_trait]
impl<Req, Res, E, H, F, S, T1> Handler<(T1, Req), S> for H
where
    Req: IncomingRequest + Send + 'static,
    Res: OutgoingResponse,
    E: IntoResponse,
    H: FnOnce(T1, Req) -> F + Clone + Send + 'static,
    F: Future<Output = Result<Res, E>> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequestParts<S> + Send + 'static,
{
    fn add_to_router(self, router: Router<S>) -> Router<S> {
        let Metadata {
            method, history, ..
        } = Req::METADATA;

        let filter = method_to_filter(method);

        history.all_paths().fold(router, |router, path| {
            let f = self.clone();

            router.route(
                path,
                on(filter, |t1: T1, Incoming(req): Incoming<Req>| async move {
                    match f(t1, req).await {
                        Ok(res) => Outgoing(res).into_response(),
                        Err(_) => todo!(),
                    }
                }),
            )
        })
    }
}
