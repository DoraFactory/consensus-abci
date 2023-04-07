use std::{
    sync::{
        Arc, RwLock, 
        atomic::{AtomicUsize, Ordering}
    }, 
    collections::HashMap
};
use tracing::info;
// use async_trait::async_trait;
use crate::{Block, SledDb, Storage, Transaction, Txoutput, error::BlockchainError};

// block difficulty
pub const CURR_BITS: usize = 5;

#[derive(Debug, Default, Clone)]
pub struct Blockchain<T = SledDb> {
    pub storage: Arc<T>,
    pub app_hash: Arc<RwLock<String>>,
    pub height: Arc<AtomicUsize>,
}

// #[async_trait]
impl<T: Storage> Blockchain<T> {
    pub async fn new(storage: Arc<T>) -> Self {
        if let Ok(Some(app_hash)) = storage.get_app_hash().await {
            let height = storage.get_height().await.unwrap();
            Self {
                storage,
                app_hash: Arc::new(RwLock::new(app_hash)),
                height: AtomicUsize::new(height.unwrap()).into(),
            }
        }else {
            Self {
                storage,
                app_hash: Arc::new(RwLock::new(String::new())),
                height: AtomicUsize::new(0).into(),
            }
        }
    }

    pub async fn create_genesis_block(&mut self, genesis_addr: &str) {
        let genesis_block = Block::create_genesis_block(CURR_BITS, genesis_addr);
        let hash = genesis_block.get_hash();
        info!("genesis hash is {:?}", hash);
        self.height.fetch_add(1, Ordering::Relaxed);
        self.storage.update_blocks(&hash, &genesis_block, self.height.load(Ordering::Relaxed)).await;
        let mut app_hash = self.app_hash.write().unwrap();
        *app_hash = hash;
    }

    pub async fn construct_block(&mut self, txs: &[Transaction]) -> Block {
        for tx in txs {
            if tx.verify(self).await == false {
                panic!("ERROR: Invalid transaction")
            }
        }

        let block = Block::new(txs, &self.app_hash.read().unwrap(), CURR_BITS);
        let hash = block.get_hash();
        self.height.fetch_add(1, Ordering::Relaxed);
        self.storage.update_blocks(&hash, &block, self.height.load(Ordering::Relaxed)).await;
        let mut app_hash = self.app_hash.write().unwrap();
        *app_hash = hash;

        block
    }

    pub async fn add_block(&mut self, block: Block) -> Result<(), BlockchainError> {
        let hash = block.get_hash();
        if let Some(_) = self.storage.get_block(&hash).await? {
            info!("Block {} already exists", hash);
        }else {
            self.height.fetch_add(1, Ordering::Relaxed);
            self.storage.update_blocks(&hash, &block, self.height.load(Ordering::Relaxed)).await;
            let mut app_hash = self.app_hash.write().unwrap();
            *app_hash = hash;
        }
        Ok(())
    }

    pub async fn find_utxo(&self) -> HashMap<String, Vec<Txoutput>> {
        let mut utxo = HashMap::new();
        let mut spent_txos = HashMap::new();

        let blocks = self.storage.get_block_iter().await.unwrap();
        for block in blocks {
            for tx in block.get_tranxs() {
                for (idx, txout) in tx.get_vout().iter().enumerate() {
                    if let Some(outs) = spent_txos.get(&tx.get_id()) {
                        for out in outs {
                            if idx.eq(out) {
                                break;
                            }

                            utxo.entry(tx.get_id())
                                .and_modify(|v: &mut Vec<Txoutput>| v.push(txout.clone()))
                                .or_insert(vec![txout.clone()]);
                        }
                    }else {
                        utxo.entry(tx.get_id())
                            .and_modify(|v: &mut Vec<Txoutput>| v.push(txout.clone()))
                            .or_insert(vec![txout.clone()]);
                    }
                }

                for txin in tx.get_vin() {
                    spent_txos.entry(txin.get_txid())
                        .and_modify(|v: &mut Vec<usize>| v.push(txin.get_vout()))
                        .or_insert(vec![txin.get_vout()]);
                }
            }
        }

        utxo
    }

    pub async fn find_transaction(&self, txid: String) -> Option<Transaction> {
        let blocks = self.storage.get_block_iter().await.unwrap();
        for block in blocks {
            for tx in block.get_tranxs() {
                if tx.get_id() == txid {
                    return Some(tx);
                }
            }
        }
        None
    }

    pub async fn blocks_info(&self) {
        let blocks = self.storage.get_block_iter().await.unwrap();
        for block in blocks {
            info!("{:#?}", block);
        }
    }

    pub async fn get_blocks(&self) -> Vec<Block> {
        self.storage.get_block_iter().await.unwrap().collect()
    }

    pub async fn get_app_hash(&self) -> String {
        self.app_hash.read().unwrap().to_string()
    }

    pub async fn get_height(&self) -> usize {
        self.height.load(Ordering::Relaxed)
    }
}