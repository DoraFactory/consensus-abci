#[tokio::main]
async fn main() {
    let host = "http://127.0.0.1:26657";

    let client = reqwest::Client::new();

    let tx: u64 = 1;

    let response = client
        .get(format!("{}/broadcast_tx_commit", host))
        .query(&[("tx", tx)])
        .send()
        .await
        .unwrap();

    println!("send tx response is {:?}", response);
}
