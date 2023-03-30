#[tokio::main]
async fn main() {
    let host = "http://127.0.0.1:26657";

    let client = reqwest::Client::new();

    let tx = 1;

    /*     let response = client
    .get(format!("{}/broadcast_tx", host))
    .query(&[("tx", tx)])
    .send()
    .await.unwrap(); */

    let response = client
        .get(format!("{}/abci_query", host))
        // 查询的就是AbciQueryQuery的data和path部分
        .query(&[("data", "counter"), ("path", "")])
        .send()
        .await.unwrap();

    println!("send tx response is {:?}", response);
}
