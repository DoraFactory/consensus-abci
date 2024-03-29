use eyre::{Result, WrapErr};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

use clap::{crate_name, crate_version, App, AppSettings, ArgMatches, SubCommand};
use tokio::sync::mpsc::{channel, Receiver, unbounded_channel};

use pow_abci::{ClientApi, Engine};

pub const CHANNEL_CAPACITY: usize = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("pow_node")
        .version(crate_version!())
        .about("a minimal practice of replacing the tendermint consensus with pow adapting the abci")
        .args_from_usage("-v... 'Sets the level of verbosity")
        .subcommand(
            SubCommand::with_name("run")
            //TODO:
            // .args_from_usage("--port=<string> 'This is a abci client server'")
        )
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();
    
    match matches.subcommand() {
        ("run", Some(sub_matches)) => run().await?,
        // ("run", Some(sub_matches)) => run(sub_matches).await?,
        _ => unreachable!(),
    }
    Ok(())
}


// async fn run(sub_matches: &ArgMatches<'_>) -> Result<()> {
async fn run() -> Result<()> {

    let (tx_req, mut rx_req) = channel(CHANNEL_CAPACITY);

    // 用于和共识的ABCI接口进行通信的mpsc channel
    let (tx_abci_req, mut rx_abci_queries) = channel(CHANNEL_CAPACITY);

    let (deliver_tx, mut deliver_rx) = unbounded_channel();

    //TODO: Add genesis account
    // let _genesis_account = matches.value_of("genesis_account").unwrap();
    // expose the client port 26657
    tokio::spawn(async move {
        // let address = "127.0.0.1:26657".to_string();
        // let addr = address.parse::<SocketAddr>().unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = 26657;
        let abci_client_address = SocketAddr::new(ip, port);
        let client_api = ClientApi::new(abci_client_address, tx_abci_req, deliver_rx);
        println!("Startd ABCI client listen on: {:?}", &abci_client_address);
        warp::serve(client_api.get_routes(tx_req)).run(abci_client_address).await
    });


    // client will connect the server with 26658
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let port = 26658;
    let app_address = SocketAddr::new(ip, port);

    // let init_app_hash = vec![0];

    let mut engine = Engine::new(app_address, rx_abci_queries);

    // engine.run(rx_req).await?;
    engine.run(rx_req, deliver_tx).await?;

    Ok(())

}

