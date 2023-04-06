// #![feature(async_fn_in_trait)]

use std::collections::HashMap;

use crate::{error::BlockchainError, blocks::Block, Txoutput};
use async_trait::async_trait;
mod sleddb;

pub use sleddb::SledDb;

pub const APP_HASH_KEY: &str = "app_hash";
pub const HEIGHT: &str = "height";
pub const TABLE_OF_BLOCK: &str = "blocks";
pub const UTXO_SET: &str = "utxos";

#[async_trait]
pub trait Storage: Send + Sync + 'static {
    async fn get_app_hash(&self) -> Result<Option<String>, BlockchainError>;
    async fn get_block(&self, key: &str) -> Result<Option<Block>, BlockchainError>;
    async fn get_height(&self) -> Result<Option<usize>, BlockchainError>;
    async fn update_blocks(&self, key: &str, block: &Block, height: usize);
    async fn get_block_iter(&self) -> Result<Box<dyn Iterator<Item = Block>>, BlockchainError>;

    async fn get_utxo_set(&self) -> HashMap<String, Vec<Txoutput>>;
    async fn write_utxo(&self, txid: &str, outs: Vec<Txoutput>) -> Result<(), BlockchainError>;
    async fn clear_utxo_set(&self);
}

pub struct StorageIterator<T> {
    data: T
}

impl<T> StorageIterator<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T> Iterator for StorageIterator<T> 
where
    T: Iterator,
    T::Item: Into<Block>
{
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        self.data.next().map(|v| v.into())
    }
}