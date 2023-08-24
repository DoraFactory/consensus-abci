use crate::{QueryInfo, Transaction};

use eyre::WrapErr;
use futures::SinkExt;
use tendermint_proto::{abci::{ResponseQuery, ResponseDeliverTx}, crypto::ProofOps};
use tendermint_rpc::{
    endpoint,
    error::{Error, ErrorDetail},
    request::Wrapper as RequestWrapper,
    Code, Order, Response,
};
use tokio::spawn;
use tokio::sync::mpsc::{Sender, Receiver, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{channel as oneshot_channel, Sender as OneShotSender};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::to_vec;
use super::{EventAttributeJson, EventJson};

use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use warp::{Filter, Rejection};
// use super::MyResponseQuery;
use base64::encode;

use core::panic;
use std::net::SocketAddr;
/// Client Api which will provide a exposed port(eg:26657) for users to get and post msg
pub struct ClientApi<T> {
    // commonly: 26657 port
    abci_client_address: SocketAddr,
    req: Sender<(OneShotSender<T>, QueryInfo)>,
    deliver_rx: Arc<Mutex<UnboundedReceiver<ResponseDeliverTx>>>
}

impl<T: Send + Sync + std::fmt::Debug> ClientApi<T> {
    pub fn new(
        abci_client_address: SocketAddr,
        req: Sender<(OneShotSender<T>, QueryInfo)>,
        deliver_rx: UnboundedReceiver<ResponseDeliverTx>
    ) -> Self {
        Self {
            abci_client_address,
            req,
            deliver_rx: Arc::new(Mutex::new(deliver_rx)),
        }
    }
}

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use warp::Reply;
use core::str::Bytes;

impl ClientApi<ResponseQuery> {
    pub fn get_routes(
        self,
        tx_req: Sender<String>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {

        let route_abci = warp::path::end() // 不添加方法名作为路径，直接匹配根路径
            .and(warp::post()) // 处理 POST 请求
            .and(warp::body::json()) // 解析 JSON 数据
            // .and_then(move |json_request| handle_abci_query(json_request, tx_req.clone()));
            .and_then(move |json_request: serde_json::Value| {
                let tx_abci_queries = self.req.clone();
                let abci_tx = tx_req.clone();
                let deliver_rx_resp = self.deliver_rx.clone();

                async move {
                    let method = json_request["method"].as_str().unwrap_or_default();
                    let params = &json_request["params"];
    
                    // 这里根据 method 字段的值执行相应的处理函数，并返回结果
                    match method {
                        "abci_query" => {
                            println!("开始abci_query请求");

                            // 查询还会进行区分，通过path
                            let path = params["path"].as_str().unwrap_or_default();
                            match path {
                                "/cosmos.bank.v1beta1.Query/AllBalances" | "/cosmos.auth.v1beta1.Query/Account" => {
                                    let data = params["data"].as_str().unwrap_or_default();
                                    let prove = params["prove"].as_bool().unwrap_or_default();
            
            
                                    let req = QueryInfo {
                                        path: Some(String::from(path)),
                                        data: data.as_bytes().to_vec(),
                                        height: None,
                                        prove,
                                    };

                                    // 开始请求abci借口
                                    println!("query request: {:?}", req.clone());

                                    // 创建一个单生产者单消费者的管道，向ABCI client发送一个消息，这个消息是一个元组(管道，req)
                                    let (tx_query, rx_query) = oneshot_channel();
                                    match tx_abci_queries.send((tx_query, req.clone())).await {
                                        Ok(_) => {}
                                        Err(err) => log::error!("Error forwarding abci query: {}", err),
                                    };
                                    let resp: ResponseQuery = rx_query.await.unwrap();

                                    println!("查询的结果响应为 {:?}", resp);
                                    println!("ID is {:?}", json_request["id"]);

                                    // let result = format!("Hello, World! Your path: {}, data: {}, prove: {}", path, data, prove);
                                    Ok(warp::reply::json(&serde_json::json!({
                                        "jsonrpc": "2.0".to_string(),
                                        "result": {
                                            "response": {
                                                "code": resp.code,
                                                "log": resp.log,
                                                "info": resp.info,
                                                "index": resp.index.to_string(),
                                                "key": encode(&resp.key),
                                                "value": encode(&resp.value),
                                                "proofOps": resp.proof_ops,
                                                "height": resp.height.to_string(),
                                                "codespace": resp.codespace,
                                            },
                                        },
                                        "id": json_request["id"]
                                    })))
                                }
                                _ => {
                                    let result = format!("Hello, World!");
                                    Ok(warp::reply::json(&serde_json::json!({
                                        "jsonrpc": "2.0".to_string(),
                                        "result": result,
                                        "id": json_request["id"]
                                    })))
                                }
                            }
                            
                        }
                        "broadcast_tx_commit" => {
                            println!("开始broadcast_tx_commit请求");
                            let transaction = params["tx"].as_str().unwrap_or_default();
                            // let result = format!("Hello, World! Your transaction is : {}", transaction);
                            println!("交易数据是{:?}", transaction);

                            // // 将整个Transaction结构发送到共识层
                            if let Err(e) = abci_tx.send(transaction.to_string()).await {
                                let result = format!("ERROR IN: broadcast_tx_commit: {:?}. Err: {}",transaction, e);
                                Ok(warp::reply::json(&serde_json::json!({
                                    "jsonrpc": "2.0".to_string(),
                                    "result": result,
                                    "id": json_request["id"]
                                })))
                            } else {
                                let mut deliver_rx_guard = deliver_rx_resp.lock().await;
                                let deliver_resp = deliver_rx_guard.recv().await.unwrap();

                                let events = deliver_resp.clone().events;
                                let events_json: Vec<EventJson> = events.iter().map(|event| EventJson {
                                    r#type: event.r#type.clone(),
                                    attributes: event.attributes.iter().map(|attr| EventAttributeJson {
                                        key: base64::encode(attr.key.to_vec().as_slice()),
                                        value: base64::encode(attr.value.to_vec().as_slice()),
                                        index: attr.index,
                                    }).collect(),
                                }).collect();


                                Ok(warp::reply::json(&serde_json::json!({
                                    "jsonrpc": "2.0".to_string(),
                                    "result": {
                                        "deliver_tx": {
                                            "code": deliver_resp.code,
                                            "data": deliver_resp.data,
                                            "log": deliver_resp.log,
                                            "info": deliver_resp.info,
                                            "gas_wanted": deliver_resp.gas_wanted,
                                            "gas_used": deliver_resp.gas_used,
                                            "events": events_json,
                                            "codespace": deliver_resp.codespace,
                                        },
                                    },
                                    "id": json_request["id"]
                                })))
                            }
                        }
                        _ => {
                            // 未知的 method，返回错误或提示
                            Err(warp::reject::not_found())
                        }
                    }
                }
            });

        route_abci
    }
}
