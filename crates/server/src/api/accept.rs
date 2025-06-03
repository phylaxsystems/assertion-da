use crate::api::{
    process_request::match_method,
    types::DbRequestSender,
};

use core::convert::Infallible;
use std::sync::Arc;

use alloy::signers::local::PrivateKeySigner;
use bollard::Docker;
use http_body_util::Full;
use hyper::{
    body::Bytes,
    Error,
    Request,
};

macro_rules! rpc_response {
    (
        $status:expr,
        $body:expr
    ) => {
        Ok(hyper::Response::builder()
            .status($status)
            .body($body)
            .unwrap())
    };
}

/// Accepts an incoming HTTP request, which it responds with
/// the appropriate api call.
#[tracing::instrument(level = "info", skip_all, target = "api::accept_request")]
pub async fn accept_request<B>(
    tx: Request<B>,
    db: DbRequestSender,
    signer: &PrivateKeySigner,
    docker: Arc<Docker>,
) -> Result<hyper::Response<Full<Bytes>>, Infallible>
where
    B: hyper::body::Body<Error = Error>,
{
    tracing::debug!(target = "api::accept_request", "Incoming request");
    // Respond accordingly
    let resp = match match_method(tx, &db, signer, docker).await {
        Ok(rax) => rax,
        Err(e) => {
            let e = e.to_string();
            return rpc_response!(400, Full::new(Bytes::from(e)));
        }
    };
    rpc_response!(200, Full::new(Bytes::from(resp)))
}

/// Macros for accepting requests
#[macro_export]
macro_rules! accept {
    (
        $io:expr,
        $db:expr,
        $signer:expr,
        $docker:expr
    ) => {
        let db_c = $db.clone();
        // Bind the incoming connection to our service
        if let Err(err) = hyper::server::conn::http1::Builder::new()
            // `service_fn` converts our function in a `Service`
            .serve_connection(
                $io,
                hyper::service::service_fn(|req| {
                    let response =
                        $crate::api::accept::accept_request(req, db_c.clone(), $signer, $docker);
                    response
                }),
            )
            .with_upgrades()
            .await
        {
            tracing::error!(?err, "Error serving connection");
        }
    };
}
