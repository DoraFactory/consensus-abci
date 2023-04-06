use std::{path::Path, collections::HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;
use sled::{Db, IVec, transaction::TransactionResult};
use std::ops::DerefMut;
use async_trait::async_trait;

use crate::{Storage, error::BlockchainError, Block, utils::{deserialize, serialize}, APP_HASH_KEY, TABLE_OF_BLOCK, HEIGHT, StorageIterator, UTXO_SET, Txoutput};

#[derive(Clone)]
pub struct SledDb {
    db: Arc<Mutex<Db>>
}

impl SledDb {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            db: Arc::new(Mutex::new(sled::open(path).unwrap()))
        }
    }

    fn get_full_key(table: &str, key: &str) -> String {
        format!("{}:{}", table, key)
    }
}

#[async_trait]
impl Storage for SledDb {
    async fn get_app_hash(&self) -> Result<Option<String>, BlockchainError> {
        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();
        let result = db.get(APP_HASH_KEY)?.map(|v| deserialize::<String>(&v.to_vec()));
        result.map_or(Ok(None), |v| v.map(Some))
    }

    async fn get_block(&self, key: &str) -> Result<Option<Block>, BlockchainError> {
        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();
        let name = Self::get_full_key(TABLE_OF_BLOCK, key);
        let result = db.get(name)?.map(|v| v.into());
        Ok(result)
    }

    async fn get_height(&self) -> Result<Option<usize>, BlockchainError> {
        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();
        let result = db.get(HEIGHT)?.map(|v| deserialize::<usize>(&v.to_vec()));
        result.map_or(Ok(None), |v| v.map(Some))
    }

    async fn update_blocks(&self, key: &str, block: &Block, height: usize) {
        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();
        let _: TransactionResult<(), ()> = db.transaction(|db| {
            let name = Self::get_full_key(TABLE_OF_BLOCK, key);
            db.insert(name.as_str(), serialize(block).unwrap())?;
            db.insert(APP_HASH_KEY, serialize(key).unwrap())?;
            db.insert(HEIGHT, serialize(&height).unwrap())?;
            db.flush();
            Ok(())
        });
    }

    async fn get_block_iter(&self) -> Result<Box<dyn Iterator<Item = Block>>, BlockchainError> {
        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();

        let prefix = format!("{}:", TABLE_OF_BLOCK);
        let iter = StorageIterator::new(db.scan_prefix(prefix));
        Ok(Box::new(iter))
    }

    async fn get_utxo_set(&self) -> HashMap<String, Vec<crate::Txoutput>> {

        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();

        let mut map = HashMap::new();

        let prefix = format!("{}:", UTXO_SET);

        for item in db.scan_prefix(prefix) {
            let (k, v) = item.unwrap();
            let txid = String::from_utf8(k.to_vec()).unwrap();
            let txid = txid.split(":").collect::<Vec<_>>()[1].into();
            let outputs = deserialize::<Vec<Txoutput>>(&v.to_vec()).unwrap();

            map.insert(txid, outputs);
        }

        map
    }

    async fn write_utxo(&self, txid: &str, outs: Vec<crate::Txoutput>) -> Result<(), BlockchainError> {
        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();

        let name = format!("{}:{}", UTXO_SET, txid);
        db.insert(name, serialize(&outs)?)?;
        Ok(())
    }

    async fn clear_utxo_set(&self) {
        let db_arc = self.db.clone();
        let mut db_lock = db_arc.lock().await;
        let mut db = db_lock.deref_mut();
        
        let prefix = format!("{}:", UTXO_SET);
        db.remove(prefix).unwrap();
    }
}

impl From<IVec> for Block {
    fn from(v: IVec) -> Self {
        let result = deserialize::<Block>(&v.to_vec());
        match result {
            Ok(block) => block,
            Err(_) => Block::default(),
        }
    }
}

impl From<Result<(IVec, IVec), sled::Error>> for Block {
    fn from(result: Result<(IVec, IVec), sled::Error>) -> Self {
        match result {
            Ok((_, v)) => match deserialize::<Block>(&v.to_vec()) {
                    Ok(block) => block,
                    Err(_) => Block::default(),
            },
            Err(_) => Block::default(),
        }
    }
}