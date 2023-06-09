use std::{collections::HashMap, sync::Arc};
use tracing::info;
use crate::{Storage, Blockchain, error::BlockchainError};

#[derive(Debug, Default, Clone)]
pub struct UTXOSet<T> {
    pub storage: Arc<T>
}

impl<T: Storage> UTXOSet<T> {
    pub fn new(storage: Arc<T>) -> Self {
        Self { 
            storage
        }
    }

    pub async fn reindex(&self, bc: &Blockchain<T>) -> Result<(), BlockchainError> {
        self.storage.clear_utxo_set().await;
        let map = bc.find_utxo().await;
        for (txid, outs) in map {
            self.storage.write_utxo(&txid, outs).await?;
        }
        Ok(())
    }

    pub async fn find_spendable_outputs(&self, public_key_hash: &[u8], amount: i32) -> (i32, HashMap<String, Vec<usize>>) {
        let mut unspent_outputs = HashMap::new();
        let mut accumulated = 0;
        let utxo_set = self.storage.get_utxo_set().await;
        
        info!("utxo集合为{:?}", utxo_set);

        for (txid, outs) in utxo_set.iter() {
            for (idx, out) in outs.iter().enumerate() {
                if out.is_locked(public_key_hash) && accumulated < amount {
                    accumulated += out.get_value();
                    unspent_outputs.entry(txid.to_string())
                        .and_modify(|v: &mut Vec<usize>| v.push(idx))
                        .or_insert(vec![idx]);
                }
            }
        }

        (accumulated, unspent_outputs)
    }
}