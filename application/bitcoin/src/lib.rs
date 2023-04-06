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


/// Define a Bitcoin App State machine
#[derive(Debug, Default, Clone)]
pub struct BitcoinState<T = SledDb> {
    //TODO: block_height and app_hash should be removed, because the bc has height and app_hash
/*     block_height: i64,
    app_hash: Vec<u8>, */
    // blockchain state 
    bc: Blockchain<T>,
    // state transition logix
    utxos: UTXOSet<T>,
}