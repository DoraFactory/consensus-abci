use clap::{App, Arg};
use once_cell::sync::Lazy;
use pow_abci::Wallets;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

static WALLET_MAP: Lazy<Arc<Mutex<HashMap<String, String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

#[tokio::main]
async fn main() {
    let matches = App::new("wallet cli")
        .version("1.0")
        .author("bun")
        .about("create account")
        .arg(
            Arg::with_name("create")
                .help("create a new address")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let wallet_name = matches.value_of("create").unwrap();
    let address = create_wallet(String::from(wallet_name)).await;
    println!("Hi {:?}, you new address is {:?}", wallet_name, address);
}

async fn create_wallet(wallet_name: String) -> String {
    let mut wallet_app = WALLET_MAP.lock().await;
    let addr = wallet_app.entry(wallet_name.clone()).or_insert_with(|| {
        let mut wallets = Wallets::new().unwrap();
        let addr = wallets.create_wallet();
        addr
    });

    addr.to_string()
}
