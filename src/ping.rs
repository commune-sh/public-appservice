use ruma::{OwnedTransactionId, TransactionId};

use std::sync::Arc;
use tokio::sync::RwLock;


#[derive(Debug, Clone)]
pub struct TxnStore {
    current_id: Arc<RwLock<Option<OwnedTransactionId>>>,
}

impl Default for TxnStore {
    fn default() -> Self {
        Self {
            current_id: Arc::new(RwLock::new(None)),
        }
    }
}

impl TxnStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn generate_txn_id(&self) -> OwnedTransactionId {
        let txn_id = TransactionId::new();

        *self.current_id.write().await = Some(txn_id.clone());

        txn_id
    }

    pub async fn verify_txn_id(&self, txn_id: &TransactionId) -> bool {
        let mut current_id = self.current_id.write().await;

        if current_id.as_ref().filter(|id| &**id == txn_id).is_some() {
            *current_id = None;

            true
        } else {
            false
        }
    }
}
