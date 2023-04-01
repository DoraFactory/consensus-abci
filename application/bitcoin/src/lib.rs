pub mod abci_server;
pub use abci_server::{ConsensusConnection, MempoolConnection, InfoConnection, SnapshotConnection};
/// Define a counter
#[derive(Debug, Default, Clone)]
pub struct BitcoinState {
    block_height: i64,
    app_hash: Vec<u8>,
    counter: u64,
}