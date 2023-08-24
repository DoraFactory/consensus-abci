mod api_server;
mod abci_engine;

mod engines;
mod wallets;
mod utils;
mod error;

pub use api_server::ClientApi;
pub use abci_engine::Engine;
pub use engines::*;
pub use wallets::*;
pub use utils::*;

use serde::{Deserialize, Serialize};
use bincode::{serialize, deserialize};
use tendermint_proto::{abci::ResponseQuery, crypto::{ProofOps, ProofOp}};
// define the request type

// https://github.com/tendermint/tendermint/blob/main/spec/abci/abci.md#delivertx-1
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    from: String,
    to: String,
    amount: String
}

// https://github.com/tendermint/tendermint/blob/main/spec/abci/abci.md#query-1
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryInfo {
    path: Option<String>,
    data: Vec<u8>,
    height: Option<u64>,
    prove: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxCommitInfo {
    
}


// 定义事件属性结构
#[derive(Serialize, Deserialize)]
pub struct EventAttributeJson {
    key: String,
    value: String,
    index: bool,
}

// 定义事件结构
#[derive(Serialize, Deserialize)]
pub struct EventJson {
    r#type: String,
    attributes: Vec<EventAttributeJson>,
}

impl Transaction {
    // 序列化为 Vec<u8>
    pub fn to_bytes(&self) -> Vec<u8> {
        serialize(self).unwrap()
    }

    // 从 Vec<u8> 反序列化
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match deserialize(bytes) {
            Ok(tx) => Some(tx),
            Err(_) => None,
        }
    }
}
