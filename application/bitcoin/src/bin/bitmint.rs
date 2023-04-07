use std::{
    env::{self, current_dir},
    io::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Sub,
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;

use tracing::{subscriber::set_global_default, Level};
use tracing_subscriber::FmtSubscriber;

use abci::async_api::Server;
use bitcoin::{ConsensusConnection, InfoConnection, MempoolConnection, SnapshotConnection};

use anyhow::Result;
use bitcoin::{NodeState, SledDb, Storage};

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
            SubCommand::with_name("initChain")
                .about("Create a new chain")
                .args_from_usage("--account=<string> 'This is a tag for the bitmint blockchain builder to set the initial account who hold the all assets'")
                .args_from_usage("--db=<string> This is the data dir for the current peer")
                .args_from_usage("--port=<string> This is a setting of ABCI server(app) port, default is 26658 and you can customize it.")
        )
/*         .subcommand(
            SubCommand::with_name("tx")
                    .about("This is a setting of")
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
        ) */
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    match matches.subcommand() {
        ("initChain", Some(matches)) => {
            println!("{:?}", matches.value_of("db"));
            let path = matches.value_of("db").unwrap();
            let genesis_account = matches.value_of("account").unwrap();
            let abci_server_port = matches.value_of("port").unwrap();

            // create peer db
            let path = current_dir().unwrap().join(String::from(path));
            let db = Arc::new(SledDb::new(path));

            // abci server port
            let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
            let port = abci_server_port.parse::<u16>().unwrap();
            let abci_server_address = SocketAddr::new(ip, port);

            run(db, genesis_account, abci_server_address, &matches).await;
        },
        _ => unreachable!(),
    }

    Ok(())
}

pub async fn run<T: Storage + std::clone::Clone>(
    storage: Arc<T>,
    genesis_account: &str,
    address: SocketAddr,
    matches: &ArgMatches<'_>,
) -> Result<()> {
    let mut node = NodeState::new(storage, genesis_account).await.unwrap();
    // start the peer node
    node.start::<T>(&matches, address).await.unwrap();
    Ok(())
}
