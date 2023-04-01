mod client_api;
mod abci_engine;

pub use client_api::ClientApi;
pub use abci_engine::Engine;

use serde::{Deserialize, Serialize};

// define the request type

// https://github.com/tendermint/tendermint/blob/main/spec/abci/abci.md#delivertx-1
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BroadcastTx {
    tx: String,
}

// https://github.com/tendermint/tendermint/blob/main/spec/abci/abci.md#query-1
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryInfo {
    path: String,
    data: String,
    height: Option<usize>,
    prove: Option<bool>,
}