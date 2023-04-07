use std::{os::{macos::raw::stat, unix::prelude::OsStrExt}, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use reqwest::Response;
use tokio::{sync::Mutex, time::sleep};


use abci::{
    async_api::{Consensus, Info, Mempool, Snapshot},
    async_trait,
    types::*,
};
use tracing::info;

// use crate::T;
use crate::{NodeState, SledDb, Storage};

/// consensus connection
// #[derive(Debug)]
pub struct ConsensusConnection<T= SledDb> where T: std::clone::Clone{
    committed_state: Arc<Mutex<NodeState<T>>>,
    current_state: Arc<Mutex<Option<NodeState<T>>>>,
}

impl<T: Clone + Send + Sync> ConsensusConnection<T> {
    pub fn new(
        committed_state: Arc<Mutex<NodeState<T>>>,
        current_state: Arc<Mutex<Option<NodeState<T>>>>,
    ) -> Self {
        Self {
            committed_state,
            current_state,
        }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync> Consensus for ConsensusConnection<T> {
    async fn init_chain(&self, _init_chain_request: RequestInitChain) -> ResponseInitChain {
        //TODO: 这里后续需要改动，需要把创建交易,而且一开始创世的时候，不做工作量证明，直接出块.........
        // 现在的做法是想直接从已有的块中进行获取，作为最新块，但是不可取，这里会出现 `when chain is already initialized`

        let mut current_state_lock = self.current_state.lock().await;
        let mut current_state = current_state_lock.as_mut().unwrap();

        // update the latest block hash, in here, this is genesis block hash
        let app_hash = Arc::clone(&current_state.bc.app_hash).read().unwrap().clone().as_bytes().to_vec();
        println!("init chain in abci server");
        
        ResponseInitChain{
            app_hash,
            ..Default::default()
        }
    }

    async fn begin_block(&self, _begin_block_request: RequestBeginBlock) -> ResponseBeginBlock {
        Default::default()
    }

    async fn deliver_tx(&self, deliver_tx_request: RequestDeliverTx) -> ResponseDeliverTx {
        println!("{:?}", deliver_tx_request.tx.clone());
        /* let new_counter = parse_bytes_to_counter(&deliver_tx_request.tx);

        if new_counter.is_err() {
            return ResponseDeliverTx {
                code: 1,
                codespace: "Parsing error".to_owned(),
                log: "Transaction should be 8 bytes long".to_owned(),
                info: "Transaction is big-endian encoding of 64-bit integer".to_owned(),
                ..Default::default()
            };
        }

        let new_counter = new_counter.unwrap(); */

        let mut current_state_lock = self.current_state.lock().await;
        let mut current_state = current_state_lock.as_mut().unwrap();

        /* if current_state.counter + 1 != new_counter {
            return ResponseDeliverTx {
                code: 2,
                codespace: "Validation error".to_owned(),
                log: "Only consecutive integers are allowed".to_owned(),
                info: "Numbers to counter app should be supplied in increasing order of consecutive integers staring from 1".to_owned(),
                ..Default::default()
            };
        }

        current_state.counter = new_counter; */

        // println!("新的count的状态为{:?}", new_counter.clone());

        //TODO: 这个需要修改一下，返回一些详细的信息
        // Default::default()
        /* ResponseDeliverTx {
            code: 0,
            codespace: "Validation successfully".to_owned(),
            log: "Updated the state with new counter".to_owned(),
            info: "Number has been increased!".to_owned(),
            ..Default::default()
        } */
        Default::default()
    }

    async fn end_block(&self, end_block_request: RequestEndBlock) -> ResponseEndBlock {
        let mut current_state_lock = self.current_state.lock().await;
        let mut current_state = current_state_lock.as_mut().unwrap();

        let end_block_height =  end_block_request.height as usize;

        current_state.bc.height = AtomicUsize::new(end_block_height).into();
        let app_hash = Arc::clone(&current_state.bc.app_hash).read().unwrap().clone();

        // get the current app hash(block hash) return to abci client by the events
        let event = Event {
            r#type: "".to_owned(),
            attributes: vec![EventAttribute {
                key: "app_hash".as_bytes().to_vec(),
                value: app_hash.as_bytes().to_owned(),
                index: true,
            }],
        };
        // Default::default()
        ResponseEndBlock {
            events: vec![event],
            ..Default::default()
        }
    }

    async fn commit(&self, _commit_request: RequestCommit) -> ResponseCommit {
/*         let current_state = self.current_state.lock().await.as_ref().unwrap().clone();
        let mut committed_state = self.committed_state.lock().await;
        *committed_state = *current_state; */

/*         ResponseCommit {
            data: Arc::new(committed_state.bc.app_hash.clone()).read().unwrap().clone().as_bytes().to_vec(),
            retain_height: 0,
        } */
        Default::default()
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

        Default::default()
    }
}

/// Info connection
pub struct InfoConnection<T= SledDb> where T: std::clone::Clone{
    state: Arc<Mutex<NodeState<T>>>,
}

impl<T: Clone + Send + Sync> InfoConnection<T> {
    pub fn new(state: Arc<Mutex<NodeState<T>>>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync> Info for InfoConnection<T> {
    async fn info(&self, _info_request: RequestInfo) -> ResponseInfo {
        let state = self.state.lock().await;
        info!("-------------This is abci server Info connection response-------------");
        ResponseInfo {
            data: "服务器已经收到您的info消息, 现在返回给您一个成功的响应!".to_string(),
            version: Default::default(),
            app_version: Default::default(),
            last_block_height:  (*state).bc.height.load(Ordering::SeqCst) as i64,
            last_block_app_hash: Arc::clone(&(*state).bc.app_hash.clone()).read().unwrap().clone().as_bytes().to_vec(),
        }
    }

    async fn query(&self, query_request: RequestQuery) -> ResponseQuery {
        let state = self.state.lock().await;
        let key = match std::str::from_utf8(&query_request.data) {
            Ok(s) => s,
            Err(e) => panic!("Failed to intepret key as UTF-8: {e}"),
        };
        
        info!("用户想要查询的是:{:?}",key);

        // 判断用户查询的是什么，先用最新的区块哈希做判断
        if key == "latest_block_hash" {
            let app_hash = &*(*state).bc.app_hash.read().unwrap();
            return ResponseQuery { 
                code: 0,
                log: "exists".to_string(),
                info: "".to_string(),
                index: 0,
                key: key.into(),
                value: app_hash.clone().into_bytes(),
                proof_ops: None,
                height: (*state).bc.height.load(Ordering::SeqCst) as i64,
                codespace: "".to_string(),
            }
        }

        Default::default()
    }
}

/// Snapshot connection
pub struct SnapshotConnection;

#[async_trait]
impl Snapshot for SnapshotConnection {}