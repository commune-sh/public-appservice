use std::sync::Arc;

use axum::extract::State;
use ruma::api::appservice::ping::send_ping;

use crate::{error::serve::Result, Application};

pub async fn send_ping_route(
    State(app): State<Arc<Application>>,
    request: send_ping::v1::Request,
) -> Result<send_ping::v1::Response> {
    let Some(txn_id) = request.transaction_id.as_ref() else {
        unimplemented!();
    };

    if !app.txn_store.verify_txn_id(&**txn_id).await {
        println!("Transaction ID does not match: {txn_id}");
    }

    Ok(send_ping::v1::Response::new())
}
