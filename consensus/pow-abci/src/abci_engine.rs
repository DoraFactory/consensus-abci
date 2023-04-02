use std::net::SocketAddr;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender as OneShotSender;

use crate::QueryInfo;

use tendermint_abci::{Client as AbciClient, ClientBuilder};
use tendermint_proto::abci::{
    RequestBeginBlock, RequestDeliverTx, RequestEcho, RequestEndBlock, RequestInfo,
    RequestInitChain, RequestQuery, ResponseQuery, Event, EventAttribute
};
use tendermint_proto::types::Header;

pub struct Engine {
    pub app_address: SocketAddr,
    pub rx_abci_queries: Receiver<(OneShotSender<ResponseQuery>, QueryInfo)>,
    pub last_block_height: i64,
    pub last_app_hash: Vec<u8>,
    pub client: AbciClient,
    pub req_client: AbciClient,
}

impl Engine {
    pub fn new(
        app_address: SocketAddr,
        rx_abci_queries: Receiver<(OneShotSender<ResponseQuery>, QueryInfo)>,
        last_app_hash: Vec<u8>,
    ) -> Self {
        let mut echo_client = ClientBuilder::default().connect(&app_address).unwrap();

        let last_block_height = echo_client
            .info(RequestInfo::default())
            .map(|res| res.last_block_height)
            .unwrap_or_default();

        // Instantiate a new client to not be locked in an Info connection
        let client = ClientBuilder::default().connect(&app_address).unwrap();
        let req_client = ClientBuilder::default().connect(&app_address).unwrap();
        Self {
            app_address,
            rx_abci_queries,
            last_block_height,
            client,
            req_client,
            last_app_hash,
        }
    }

    // pub async fn run(&mut self, mut rx_output: Receiver<u64>) -> eyre::Result<()> {
    pub async fn run(&mut self, mut rx_output: Receiver<u64>) -> eyre::Result<()> {
        self.init_chain()?;
        
        loop {
            println!("listening and consuming the coming requests....");
            tokio::select! {
                Some(count) = rx_output.recv() => {
                    println!("--------------------------------");
                    println!("send transaction and consensus start...");
                    self.handle_count(count)?;
                    println!("--------------------------------");

                },
                Some((tx_query, req)) = self.rx_abci_queries.recv() => {
                    println!("********************************");
                    println!("query start....");
                    self.handle_abci_query(tx_query, req)?;
                    println!("********************************");
                }
                else => break,
            }
        }
        Ok(())
    }

    fn handle_count(&mut self, count: u64) -> eyre::Result<()> {
        // increment block
        let proposed_block_height = self.last_block_height + 1;

        self.last_block_height = proposed_block_height;

        self.begin_block(proposed_block_height)?;

        self.aggrement_tx(count)?;

        self.end_block(proposed_block_height)?;

        println!("交易发送成功,当前的app hash为:{:?}", self.last_app_hash);

        self.commit()?;

        Ok(())
    }


    //TODO: 这里主要是处理共识的部分，如果要加区块链的共识，就修改这部分的逻辑
    fn aggrement_tx(&mut self, count: u64) -> eyre::Result<()> {
        //TODO: 达到目标难度值，然后打包，将交易deliver
        self.deliver_tx(count)
    }


    fn handle_abci_query(
        &mut self,
        tx_query: OneShotSender<ResponseQuery>,
        req: QueryInfo,
    ) -> eyre::Result<()> {
        let req_height = req.height.unwrap_or(0);
        let req_prove = req.prove.unwrap_or(false);

        // abci client call the `query` abci api
        let resp = self.req_client.query(RequestQuery {
            data: req.data.into(),
            path: req.path,
            height: req_height as i64,
            prove: req_prove,
        })?;

        // info test
/*         let resp = self.req_client.info(RequestInfo {
            version: Default::default(),
            block_version: Default::default(),
            p2p_version: Default::default(),
        });

        println!("在这里获取查询响应: {:?}", resp.unwrap()); */

        // 通过一个单通道单消费者(oneshoter)向用户返回响应结果

        println!("the received response is: {:?}", resp);

        let resp_key = match std::str::from_utf8(&resp.key) {
            Ok(s) => s,
            Err(e) => panic!("Failed to intepret key as UTF-8: {e}"),
        };
        println!("resp key为:{:?}", resp_key);


        let resp_value = match std::str::from_utf8(&resp.value) {
            Ok(s) => s,
            Err(e) => panic!("Failed to intepret key as UTF-8: {e}"),
        };

        println!("resp value为:{:?}", resp_value);

        if let Err(err) = tx_query.send(resp) {
            eyre::bail!("{:?}", err);
        }

        Ok(())
    }
}

impl Engine {
    /// Calls the `InitChain` hook on the app, ignores "already initialized" errors.
    pub fn init_chain(&mut self) -> eyre::Result<()> {
        let mut client = ClientBuilder::default().connect(&self.app_address)?;
        //TODO: 这里在一开始需要去连接app的时候，首先需要传入一个账户作为链的初始账户
        match client.init_chain(Default::default()) {
            Ok(_) => {
                println!("Init chain successfully! Hello ABCI.")
            }
            Err(err) => {
                // ignore errors about the chain being uninitialized
                if err.to_string().contains("already initialized") {
                    log::warn!("{}", err);
                    return Ok(());
                }
                eyre::bail!(err)
            }
        };
        Ok(())
    }

    /// Calls the `BeginBlock` hook on the ABCI app. For now, it just makes a request with
    /// the new block height.
    // If we wanted to, we could add additional arguments to be forwarded from the Consensus
    // to the App logic on the beginning of each block.
    fn begin_block(&mut self, height: i64) -> eyre::Result<()> {
        let req = RequestBeginBlock {
            header: Some(Header {
                height: self.last_block_height,
                // current app hash(对于区块链来说，这里是当前最新的区块哈希)
                app_hash: self.last_app_hash.clone(),
                ..Default::default()
            }),
            ..Default::default()
        };

        self.client.begin_block(req)?;
        Ok(())
    }

    /// Calls the `DeliverTx` hook on the ABCI app.
    fn deliver_tx(&mut self, tx: u64) -> eyre::Result<()> {
        // println!("deliver的交易传入为:{:?}", tx);
        // let tx_req = parse_int_to_bytes(tx).unwrap();
        
        self.client.deliver_tx(RequestDeliverTx { tx: counter_to_bytes(tx).to_vec() })?;
        Ok(())
    }

    /// Calls the `EndBlock` hook on the ABCI app. For now, it just makes a request with
    /// the proposed block height.
    // If we wanted to, we could add additional arguments to be forwarded from the Consensus
    // to the App logic on the end of each block.
    fn end_block(&mut self, height: i64) -> eyre::Result<()> {
        let req = RequestEndBlock { height };
        let resp = self.client.end_block(req)?;
        
        // save the ened block hash(app hash) returned by the abci server
        let event: Event = resp.events.first().unwrap().clone();
        let event_attribute: EventAttribute = event.attributes.first().unwrap().clone();
        self.last_app_hash = event_attribute.value.clone();
        Ok(())
    }

    /// Calls the `Commit` hook on the ABCI app.
    fn commit(&mut self) -> eyre::Result<()> {
        self.client.commit()?;
        Ok(())
    }
}

pub fn counter_to_bytes(counter: u64) -> [u8; 8] {
    counter.to_be_bytes()
}


// pub type Transaction = Vec<u8>;