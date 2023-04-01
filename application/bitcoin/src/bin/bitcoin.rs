use std::{net::SocketAddr, sync::Arc, time::Duration};
use tracing::{subscriber::set_global_default, Level};
use tracing_subscriber::FmtSubscriber;
use tokio::sync::Mutex;

use abci::async_api::Server;
use bitcoin::{ ConsensusConnection, MempoolConnection, InfoConnection, SnapshotConnection, BitcoinState};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    set_global_default(subscriber).unwrap();

    let server = server();
    server
        .run("127.0.0.1:26658".parse::<SocketAddr>().unwrap())
        .await
}


pub fn server() -> Server<ConsensusConnection, MempoolConnection, InfoConnection, SnapshotConnection>
{
    let committed_state: Arc<Mutex<BitcoinState>> = Default::default();
    let current_state: Arc<Mutex<Option<BitcoinState>>> = Default::default();

    let consensus = ConsensusConnection::new(committed_state.clone(), current_state);
    let mempool = MempoolConnection;
    let info = InfoConnection::new(committed_state);
    let snapshot = SnapshotConnection;

    Server::new(consensus, mempool, info, snapshot)
}
