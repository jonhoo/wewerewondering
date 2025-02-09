use axum::response::IntoResponse;
use http_body_util::BodyExt;
use lambda_http::Error;
use std::{future::Future, pin::Pin};
use tower::Layer;
use tower_http::trace::TraceLayer;
use tower_service::Service;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .without_time(/* cloudwatch does that */)
        .init();

    let app = wewerewondering_api::new().await;
    // To run with AWS Lambda runtime, wrap in our `LambdaLayer`
    let app = tower::ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(LambdaLayer)
        .service(app);

    lambda_http::run(app).await
}

#[derive(Clone, Copy)]
pub struct LambdaLayer;

impl<S> Layer<S> for LambdaLayer {
    type Service = LambdaService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LambdaService { inner }
    }
}

pub struct LambdaService<S> {
    inner: S,
}

impl<S> Service<lambda_http::Request> for LambdaService<S>
where
    S: Service<axum::http::Request<axum::body::Body>>,
    S::Response: axum::response::IntoResponse + Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    type Response = lambda_http::Response<lambda_http::Body>;
    type Error = lambda_http::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: lambda_http::Request) -> Self::Future {
        let (parts, body) = req.into_parts();
        let body = match body {
            lambda_http::Body::Empty => axum::body::Body::default(),
            lambda_http::Body::Text(t) => t.into(),
            lambda_http::Body::Binary(v) => v.into(),
        };

        let request = axum::http::Request::from_parts(parts, body);

        let fut = self.inner.call(request);
        let fut = async move {
            let resp = fut.await?;
            let (parts, body) = resp.into_response().into_parts();
            let bytes = body.collect().await?.to_bytes();
            let bytes: &[u8] = &bytes;
            let resp: hyper::Response<lambda_http::Body> = match std::str::from_utf8(bytes) {
                Ok(s) => hyper::Response::from_parts(parts, s.into()),
                Err(_) => hyper::Response::from_parts(parts, bytes.into()),
            };
            Ok(resp)
        };

        Box::pin(fut)
    }
}
