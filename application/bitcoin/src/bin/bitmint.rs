use std::{
    env::{self, current_dir},
    net::{SocketAddr, IpAddr, Ipv4Addr},
    ops::Sub,
    sync::Arc,
    time::Duration,
    io::Error
};
use tokio::sync::Mutex;

use tracing::{subscriber::set_global_default, Level};
use tracing_subscriber::FmtSubscriber;

use abci::async_api::Server;
use bitcoin::{
    BitcoinState, ConsensusConnection, InfoConnection, MempoolConnection, SnapshotConnection,
};

use anyhow::{Result};
use bitcoin::{Node, SledDb};

use clap::{crate_authors, crate_name, crate_version, App, AppSettings, ArgMatches, SubCommand};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    set_global_default(subscriber).unwrap();

    // tracing_subscriber::fmt::init();

    let matches = App::new(crate_name!())
        .version(crate_name!())
        .about("A implement of bitcoin by the tendermint ABCI, which we called bitmint blockchain")
        .arg_from_usage("-v... 'Sets the level of verbosity'")
        .subcommand(
            SubCommand::with_name("data-dir")
                .about("This is the data dir for the current peer")
        )
        .subcommand(
            SubCommand::with_name("server")
                .about("This is a setting of ABCI server(app) port, default is 26658 and you can customize it.")
        )
        //TODO: 上面都是启动一个链的重要操作
        .subcommand(
            SubCommand::with_name("genesis")
                .about("To see if the genesis block was created in bitmint.")
        )
        .subcommand(
            SubCommand::with_name("blocks")
                .about("query the all blocks in bitmint.")
        )
        .subcommand(
            SubCommand::with_name("create_wallet")
                .about("create a new bitcoin wallet.")
        )
        .subcommand(
            SubCommand::with_name("transfer_tx")
                .about("transfer from one user to another user")
                .args_from_usage("--from=<String>> 'The sender address in bitmint.'")
                .args_from_usage("--to=<String>> 'The receiver address in bitmint.'")
                .args_from_usage("--amount=<String> 'The amount of this transfer'")
        )

        // sync应该是一个一直持续的动作,可以放在node那边持续进行
        /* .subcommand(
            SubCommand::with_name("sync")
        ) */
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    let path = matches.value_of("data-dir").unwrap();
    let abci_server_port = matches.value_of("server").unwrap();


    // 创建本地peer节点数据存储
    let path = current_dir().unwrap().join(String::from(path));
    let db = Arc::new(SledDb::new(path));
    let mut node = Node::new(db).await.unwrap();

    // start the peer node
    node.start(&matches).await;

    let server = server();

    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let port = abci_server_port.parse::<u16>().unwrap();
    // TODO: 这里的端口还可以做设置（对于非本地的测试，默认都是26658端口）
    let abci_server_address = SocketAddr::new(ip, port);

    // start the abci server
    server
        .run(abci_server_address)
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
