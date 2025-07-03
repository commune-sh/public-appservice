use axum::{Json, extract::State, response::IntoResponse};

use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::error::AppserviceError;

#[derive(Debug, Clone)]
pub struct TransactionStore {
    current_id: Arc<RwLock<Option<String>>>,
}

impl Default for TransactionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            current_id: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn generate_transaction_id(&self) -> String {
        let transaction_id = Uuid::new_v4().to_string();
        let mut store = self.current_id.write().await;
        *store = Some(transaction_id.clone());
        transaction_id
    }

    pub async fn verify_and_remove_transaction(&self, transaction_id: &str) -> bool {
        let mut store = self.current_id.write().await;
        if let Some(stored_id) = store.as_ref() {
            if stored_id == transaction_id {
                *store = None;
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PingRequest {
    pub transaction_id: String,
}

pub async fn ping(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PingRequest>,
) -> Result<impl IntoResponse, AppserviceError> {
    let txn_id = payload.transaction_id.clone();

    if !state
        .transaction_store
        .verify_and_remove_transaction(&txn_id)
        .await
    {
        tracing::info!("Transaction ID does not match: {}", txn_id);
    }

    Ok(Json(json!({})))
}
