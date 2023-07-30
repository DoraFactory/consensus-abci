use std::net::SocketAddr;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender as OneShotSender;
use tendermint_proto::Protobuf;
use crate::{pow::ProofOfWork, QueryInfo, Transaction};
use tracing::info;
use tendermint_abci::{Client as AbciClient, ClientBuilder};
use tendermint_proto::abci::{
    Event, EventAttribute, RequestBeginBlock, RequestDeliverTx, RequestEcho, RequestEndBlock,
    RequestInfo, RequestInitChain, RequestQuery, ResponseQuery,
};
use tendermint_proto::types::Header;
use tendermint_proto::serializers::bytes;
use tendermint_rpc::{
    endpoint,
    error::{Error, ErrorDetail},
    request::Wrapper as RequestWrapper,
    Code, Order, Response,
};
pub const DIFFICULTY: usize = 10;

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
        // last_app_hash: Vec<u8>,
    ) -> Self {
        let mut echo_client = ClientBuilder::default().connect(&app_address).unwrap();

        let resp_info = echo_client.info(RequestInfo::default()).unwrap();
        let last_block_height = resp_info.last_block_height;
        let last_app_hash = resp_info.last_block_app_hash;

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

    pub async fn run(&mut self, mut rx_output: Receiver<String>) -> eyre::Result<()> {
        // TODO: 如果高度大于1，那就不init chain
        self.init_chain()?;

        loop {
            println!("listening and consuming the coming requests....");
            tokio::select! {
                Some(transaction) = rx_output.recv() => {
                    println!("--------------------------------");
                    println!("send transaction and consensus start...");
                    self.handle_tx(transaction)?;
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

    fn handle_tx(&mut self, trans: String) -> eyre::Result<()> {
        // increment block
        let proposed_block_height = self.last_block_height + 1;

        self.last_block_height = proposed_block_height;

        self.begin_block(proposed_block_height)?;

        self.aggrement_tx(trans)?;

        self.end_block(proposed_block_height)?;

        self.commit()?;

        println!("交易发送成功,当前的app hash为:{:?}", self.last_app_hash);

        Ok(())
    }

    // 这里主要是处理共识的部分，如果要加区块链的共识，就修改这部分的逻辑
    fn aggrement_tx(&mut self, trans: String) -> eyre::Result<()> {
        // 达到目标难度值，然后打包，将交易deliver
        // step1：先计算达到目标难度
        let pow = ProofOfWork::new(DIFFICULTY);
        println!("创建了一个pow的难题,现在开始计算");
        pow.run(trans.clone());

        // step2: 得到一个batch区块(此块非blockchain的块，而是一个节点先打包的块，需要交给app对其中的交易进行状态转换)
        // 这里暂时没有写mempool，所以会把发送过来的一笔交易直接打包为块，发送出去
        self.deliver_tx(trans)
    }

    fn handle_abci_query(
        &mut self,
        tx_query: OneShotSender<ResponseQuery>,
        req: QueryInfo,
    ) -> eyre::Result<()> {
        let req_height = req.height.unwrap_or(0);
        let req_prove = req.prove;

        /* 
        Incoming request: Request { value: Some(Query(RequestQuery { data: b"\n-cosmos1syavy2npfyt9tcncdtsdzf7kny9lh777pahuux", path: "/cosmos.bank.v1beta1.Query/AllBalances", height: 0, prove: false })) }
        Incoming request: Request { value: Some(Query(RequestQuery { data: b"0A2D636F736D6F73317379617679326E706679743974636E63647473647A66376B6E79396C68373737706168757578", path: "/cosmos.bank.v1beta1.Query/AllBalances", height: 0, prove: false })) }
         */
        let data = hex::decode(req.data).unwrap();
        println!("data: {:?}", data);
        // abci client call the `query` abci api
        let resp = self.req_client.query(RequestQuery {
            data,
            path: req.path.unwrap(),
            height: req_height as i64,
            prove: req_prove,
        })?; 

        // 通过一个单通道单消费者(oneshoter)向用户返回响应结果
        println!("the received response is: {:?}", resp);

        let resp_key = match std::str::from_utf8(&resp.key) {
            Ok(s) => s,
            Err(e) => panic!("Failed to intepret key as UTF-8: {e}"),
        };
        println!("resp key为:{:?}", resp_key);

        println!("resp value为 :{:?}", resp.clone().value);

        if let Err(err) = tx_query.send(resp) {
            eyre::bail!("{:?}", err);
        } 

        Ok(())
    }
}

impl Engine {
    /// Calls the `InitChain` hook on the app, ignores "already initialized" errors.
    pub fn init_chain(&mut self) -> eyre::Result<()> {
        // 如果之前已经启动过，那么不会进行初始化
        if self.last_block_height > 1 {
            println!("start from the exist block data.....");
            println!("当前区块高度为:{:?}", self.last_block_height);
        } else {
            let mut client = ClientBuilder::default().connect(&self.app_address).unwrap();
            println!("start init chain request");
            //TODO: 后续需要做改动，把初始化账户的功能放在这里（钱包和genesis account的功能是应该放在共识层的）
            /*         match client.init_chain(Default::default()) {
            /*             Ok(resp) => {
                            println!("Init chain successfully! Hello ABCI.");
                            self.last_app_hash = resp.app_hash
                        } */
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
                    };  */
    
            // let resp = client.init_chain();
            // println!("{:?}", resp);
        }
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
    fn deliver_tx(&mut self, tx: String) -> eyre::Result<()> {
        // println!("deliver的交易传入为:{:?}", tx);
        // let tx_req = parse_int_to_bytes(tx).unwrap();

        //TODO:
/*         self.client
            .deliver_tx(RequestDeliverTx { tx: tx.to_bytes() })?; */
        Ok(())
    }

    /// Calls the `EndBlock` hook on the ABCI app. For now, it just makes a request with
    /// the proposed block height.
    // If we wanted to, we could add additional arguments to be forwarded from the Consensus
    // to the App logic on the end of each block.
    fn end_block(&mut self, height: i64) -> eyre::Result<()> {
        let req = RequestEndBlock { height };
        let _resp = self.client.end_block(req)?;

        // save the ened block hash(app hash) returned by the abci server
        /*         let event: Event = resp.events.first().unwrap().clone();
        let event_attribute: EventAttribute = event.attributes.first().unwrap().clone();
        self.last_app_hash = event_attribute.value.clone(); */
        Ok(())
    }

    /// Calls the `Commit` hook on the ABCI app.
    fn commit(&mut self) -> eyre::Result<()> {
        let resp = self.client.commit().unwrap();
        let app_hash = resp.data;
        self.last_app_hash = app_hash;
        Ok(())
    }
}

pub fn counter_to_bytes(counter: u64) -> [u8; 8] {
    counter.to_be_bytes()
}

// pub type Transaction = Vec<u8>;
