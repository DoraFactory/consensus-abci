#[tokio::main]
async fn main() {
    let host = "http://127.0.0.1:26657";

    let client = reqwest::Client::new();

    let tx: u64 = 1;

    let response = client
        .get(format!("{}/abci_query", host))
        // 查询的就是AbciQueryQuery的data和path部分
        .query(&[("data", "latest_block_hash"), ("path", "")])
        .send()
        .await.unwrap();

    println!("latest block hash query response is {:?}", response);
}
