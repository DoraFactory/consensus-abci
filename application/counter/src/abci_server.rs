use std::{ sync::Arc, time::Duration, os::macos::raw::stat };
use tokio::{sync::Mutex, time::sleep};

use abci::{
    async_api::{
        Consensus, Info, Mempool,Snapshot,
    },
    async_trait,
    types::*,
};
use tracing::info;

use crate::CounterState;

/// consensus connection
#[derive(Debug)]
pub struct ConsensusConnection {
    committed_state: Arc<Mutex<CounterState>>,
    current_state: Arc<Mutex<Option<CounterState>>>,
}

impl ConsensusConnection {
    pub fn new(
        committed_state: Arc<Mutex<CounterState>>,
        current_state: Arc<Mutex<Option<CounterState>>>,
    ) -> Self {
        Self {
            committed_state,
            current_state,
        }
    }
}


#[async_trait]
impl Consensus for ConsensusConnection {
    async fn init_chain(&self, _init_chain_request: RequestInitChain) -> ResponseInitChain {
        Default::default()
    }

    async fn begin_block(&self, _begin_block_request: RequestBeginBlock) -> ResponseBeginBlock {
        let committed_state = self.committed_state.lock().await.clone();

        let mut current_state = self.current_state.lock().await;
        *current_state = Some(committed_state);

        Default::default()
    }

    async fn deliver_tx(&self, deliver_tx_request: RequestDeliverTx) -> ResponseDeliverTx {
        let new_counter = parse_bytes_to_counter(&deliver_tx_request.tx);

        if new_counter.is_err() {
            return ResponseDeliverTx {
                code: 1,
                codespace: "Parsing error".to_owned(),
                log: "Transaction should be 8 bytes long".to_owned(),
                info: "Transaction is big-endian encoding of 64-bit integer".to_owned(),
                ..Default::default()
            };
        }

        let new_counter = new_counter.unwrap();

        let mut current_state_lock = self.current_state.lock().await;
        let mut current_state = current_state_lock.as_mut().unwrap();

        if current_state.counter + 1 != new_counter {
            return ResponseDeliverTx {
                code: 2,
                codespace: "Validation error".to_owned(),
                log: "Only consecutive integers are allowed".to_owned(),
                info: "Numbers to counter app should be supplied in increasing order of consecutive integers staring from 1".to_owned(),
                ..Default::default()
            };
        }

        current_state.counter = new_counter;

        Default::default()
    }

    async fn end_block(&self, end_block_request: RequestEndBlock) -> ResponseEndBlock {
        let mut current_state_lock = self.current_state.lock().await;
        let mut current_state = current_state_lock.as_mut().unwrap();

        current_state.block_height = end_block_request.height;
        current_state.app_hash = current_state.counter.to_be_bytes().to_vec();

        Default::default()
    }

    async fn commit(&self, _commit_request: RequestCommit) -> ResponseCommit {
        let current_state = self.current_state.lock().await.as_ref().unwrap().clone();
        let mut committed_state = self.committed_state.lock().await;
        *committed_state = current_state;

        ResponseCommit {
            data: (*committed_state).app_hash.clone(),
            retain_height: 0,
        }
    }
}

/// mempool connection
#[derive(Debug)]
pub struct MempoolConnection;

#[async_trait]
impl Mempool for MempoolConnection {
    async fn check_tx(&self, check_tx_request: RequestCheckTx) -> ResponseCheckTx {
        if let CheckTxType::Recheck = check_tx_request.r#type() {
            sleep(Duration::from_secs(2)).await;
        }

        let new_counter = parse_bytes_to_counter(&check_tx_request.tx).unwrap();

        ResponseCheckTx {
            data: new_counter.to_be_bytes().to_vec(),
            ..Default::default()
        }
    }
}


/// Info connection
pub struct InfoConnection {
    state: Arc<Mutex<CounterState>>,
}

impl InfoConnection {
    pub fn new(state: Arc<Mutex<CounterState>>) -> Self {
        info!("Current State is: {:?}", state);
        Self { state }
    }
}

#[async_trait]
impl Info for InfoConnection {
    async fn info(&self, _info_request: RequestInfo) -> ResponseInfo {
        let state = self.state.lock().await;
        info!("-------------This is abci server Info connection response-------------");
        ResponseInfo {
            data: "服务器已经收到您的info消息，现在返回给您一个成功的响应！".to_string(),
            version: Default::default(),
            app_version: Default::default(),
            last_block_height: (*state).block_height,
            last_block_app_hash: (*state).app_hash.clone(),
        }
    }

    async fn query(&self, query_request: RequestQuery) -> ResponseQuery{
        let state = self.state.lock().await;
        let key = match std::str::from_utf8(&query_request.data) {
            Ok(s) => s,
            Err(e) => panic!("Failed to intepret key as UTF-8: {e}"),
        };

        info!("用户想要查询的是:{:?}",key);
        ResponseQuery { 
            code: 0,
            log: "exists".to_string(),
            info: "".to_string(),
            index: 0,
            key: key.into(),
            value: (*state).counter.to_string().as_bytes().to_vec(),
            proof_ops: None,
            height: (*state).block_height,
            codespace: "".to_string(),
        }
    }

}


/// Snapshot connection
pub struct SnapshotConnection;

#[async_trait]
impl Snapshot for SnapshotConnection {}

fn parse_bytes_to_counter(bytes: &[u8]) -> Result<u64, ()> {
    if bytes.len() != 8 {
        return Err(());
    }

    let mut counter_bytes = [0; 8];
    counter_bytes.copy_from_slice(bytes);

    Ok(u64::from_be_bytes(counter_bytes))
}

