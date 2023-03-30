use std::net::SocketAddr;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender as OneShotSender;

use crate::QueryInfo;

use tendermint_abci::{Client as AbciClient, ClientBuilder};
use tendermint_proto::abci::{
    RequestBeginBlock, RequestDeliverTx, RequestEcho, RequestEndBlock, RequestInfo,
    RequestInitChain, RequestQuery, ResponseQuery,
};
use tendermint_proto::types::Header;

pub struct Engine {
    pub app_address: SocketAddr,
    pub rx_abci_queries: Receiver<(OneShotSender<ResponseQuery>, QueryInfo)>,
    pub last_block_height: i64,
    pub client: AbciClient,
    pub req_client: AbciClient,
}

impl Engine {
    pub fn new(
        app_address: SocketAddr,
        rx_abci_queries: Receiver<(OneShotSender<ResponseQuery>, QueryInfo)>,
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
        }
    }

    pub async fn run(&mut self, mut rx_output: Receiver<i32>) -> eyre::Result<()> {
        self.init_chain()?;
        
        loop {
            println!("listening and consuming the coming requests....");
            tokio::select! {
                Some(count) = rx_output.recv() => {
                    println!("--------------------------------");
                    println!("send transaction and consensus start...");
                    println!("--------------------------------");
                    self.handle_count(count)?;
                },
                Some((tx_query, req)) = self.rx_abci_queries.recv() => {
                    println!("query start....");
                    println!("********************************");
                    self.handle_abci_query(tx_query, req)?;
                    println!("********************************");
                }
                else => break,
            }
        }
        Ok(())
    }

    fn handle_count(&mut self, count: i32) -> eyre::Result<()> {
        // increment block
        let proposed_block_height = self.last_block_height + 1;

        self.last_block_height = proposed_block_height;

        self.begin_block(proposed_block_height)?;

        self.aggrement_tx(count)?;

        self.end_block(proposed_block_height)?;

        self.commit()?;

        Ok(())
    }


    fn aggrement_tx(&mut self, count: i32) -> eyre::Result<()> {
        let bytes = parse_int_to_bytes(count).unwrap();
        self.deliver_tx(bytes)
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

        println!("获取的查询响应为: {:?}", resp);

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
        match client.init_chain(RequestInitChain::default()) {
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
                height,
                ..Default::default()
            }),
            ..Default::default()
        };

        self.client.begin_block(req)?;
        Ok(())
    }

    /// Calls the `DeliverTx` hook on the ABCI app.
    fn deliver_tx(&mut self, tx: [u8;8]) -> eyre::Result<()> {
        self.client.deliver_tx(RequestDeliverTx { tx: tx.to_vec() })?;
        Ok(())
    }

    /// Calls the `EndBlock` hook on the ABCI app. For now, it just makes a request with
    /// the proposed block height.
    // If we wanted to, we could add additional arguments to be forwarded from the Consensus
    // to the App logic on the end of each block.
    fn end_block(&mut self, height: i64) -> eyre::Result<()> {
        let req = RequestEndBlock { height };
        self.client.end_block(req)?;
        Ok(())
    }

    /// Calls the `Commit` hook on the ABCI app.
    fn commit(&mut self) -> eyre::Result<()> {
        self.client.commit()?;
        Ok(())
    }
}

pub fn parse_int_to_bytes(count: i32) -> Result<[u8; 8], ()>{
    let num: i32 = 42;
    let bytes: [u8; 8] = {
        let mut arr = [0u8; 8];
        let num_bytes = num.to_le_bytes();
        arr[..4].copy_from_slice(&num_bytes);
        arr[4..].copy_from_slice(&num_bytes);
        arr
    };
    Ok(bytes)
}


// pub type Transaction = Vec<u8>;