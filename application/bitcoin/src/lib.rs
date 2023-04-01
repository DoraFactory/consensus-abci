mod abci_server;
pub use abci_server::{ConsensusConnection, MempoolConnection, InfoConnection, SnapshotConnection};

mod error;
mod utils;
mod blocks;
mod storage;
mod transactions;
mod wallets;
mod networks;

pub use blocks::*;
pub use storage::*;
pub use transactions::*;
pub use wallets::*;
pub use networks::*;


/// Define a counter
#[derive(Debug, Default, Clone)]
pub struct BitcoinState {
    block_height: i64,
    app_hash: Vec<u8>,
    counter: u64,
}