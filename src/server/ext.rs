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

pub trait RouterExt<S> {
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

pub trait Handler<T, S> {
    fn add_to_router(self, router: Router<S>) -> Router<S>;
}

macro_rules! impl_ruma_handler {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        #[async_trait]
        impl<Req, Res, E, H, F, S, $( $ty, )*> Handler<($($ty,)* Incoming<Req>,), S> for H
        where
            Req: IncomingRequest + Send + 'static,
            Res: OutgoingResponse,
            E: IntoResponse,
            H: FnOnce($($ty,)* Req) -> F + Clone + Send + 'static,
            F: Future<Output = Result<Res, E>> + Send,
            S: Clone + Send + Sync + 'static,
            $( $ty: FromRequestParts<S> + Send + 'static, )*
        {
            fn add_to_router(self, router: Router<S>) -> Router<S> {
                let Metadata {
                    method, history, ..
                } = Req::METADATA;

                let method = match method {
                    Method::DELETE => MethodFilter::DELETE,
                    Method::GET => MethodFilter::GET,
                    Method::HEAD => MethodFilter::HEAD,
                    Method::OPTIONS => MethodFilter::OPTIONS,
                    Method::PATCH => MethodFilter::PATCH,
                    Method::POST => MethodFilter::POST,
                    Method::PUT => MethodFilter::PUT,
                    Method::TRACE => MethodFilter::TRACE,
                    m => panic!("Unsupported HTTP method: {m:?}"),
                };

                history.all_paths().fold(router, |router, path| {
                    let f = self.clone();

                    router.route(
                        path,
                        on(method, |$( $ty: $ty, )* req: Incoming<Req>| async move {
                            match f($( $ty, )* req.0).await {
                                Ok(res) => Outgoing(res).into_response(),
                                Err(error) => error.into_response(),
                            }
                        }),
                    )
                })
            }
        }
    };
}

impl_ruma_handler!();
impl_ruma_handler!(T1);
impl_ruma_handler!(T1, T2);
impl_ruma_handler!(T1, T2, T3);
impl_ruma_handler!(T1, T2, T3, T4);
impl_ruma_handler!(T1, T2, T3, T4, T5);
impl_ruma_handler!(T1, T2, T3, T4, T5, T6);
impl_ruma_handler!(T1, T2, T3, T4, T5, T6, T7);
impl_ruma_handler!(T1, T2, T3, T4, T5, T6, T7, T8);
