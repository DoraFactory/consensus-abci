use clap::{App, Arg};
use reqwest::Error;

#[tokio::main]
async fn main() {
    let matches = App::new("transfer cli")
        .version("1.0")
        .author("bun")
        .about("transfer bitcoin")
        .arg(
            Arg::with_name("from")
                .help("The source address")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("to")
                .help("The target address")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("amount")
                .help("The amount of this balance transfer")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let from = matches.value_of("from").unwrap();
    let to = matches.value_of("to").unwrap();
    let amount = matches.value_of("amount").unwrap();
    println!("from:{:?},to {:?}, amount:{:?}", from, to, amount);
    send_request(from, to, amount).await;
}

async fn send_request(from: &str, to: &str, amount: &str) {
    let url = "http://127.0.0.1:26657";

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/broadcast_tx_commit", url))
        .query(&[("from", from), ("to", to), ("amount", amount)])
        .send()
        .await
        .unwrap();

    println!("send tx response is {:?}", response);
    
}
