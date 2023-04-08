use super::{create_swarm, BLOCK_TOPIC, PEER_ID, TRANX_TOPIC, WALLET_MAP};
use crate::{
    blocks::Blockchain, Block, BlockchainBehaviour, Commands, Messages, SledDb, Storage,
    Transaction, UTXOSet, Wallets,
};
use anyhow::{Error, Result};
use clap::{crate_name, crate_version, App, AppSettings, ArgMatches, SubCommand};
use futures::{task::AtomicWaker, FutureExt, StreamExt};
use libp2p::{swarm::SwarmEvent, PeerId, Swarm};
use std::io;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::Arc;
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    spawn,
    sync::{mpsc, Mutex},
};
use tracing::{error, info};

use crate::{ConsensusConnection, InfoConnection, MempoolConnection, SnapshotConnection};
use abci::async_api::Server;

// Node主要处理p2p网络同步
#[derive(Clone)]
pub struct NodeState<T = SledDb>
where
    T: std::clone::Clone,
{
    pub bc: Blockchain<T>,
    pub utxos: UTXOSet<T>,
    pub msg_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Messages>>>,
    pub swarm: Arc<Mutex<Swarm<BlockchainBehaviour>>>,
}

impl<T: Storage + std::clone::Clone> NodeState<T> {
    pub async fn new(storage: Arc<T>, genesis_account: &str) -> Result<Self> {
        let (msg_sender, msg_receiver) = mpsc::unbounded_channel();

        let walle_name = String::from(genesis_account);
        let mut wallet_app = WALLET_MAP.lock().await;

        // TODO: should be removed
        let addr = wallet_app.entry(walle_name.clone()).or_insert_with(|| {
            let mut wallets = Wallets::new().unwrap();
            let addr = wallets.create_wallet();
            info!("{}'s address is {}", walle_name, addr);
            addr
        });

        //TODO: 如果是非第一个节点，直接同步区块就行，不需要创建

        let mut bc = Blockchain::new(storage.clone()).await;
        info!("setup blockchain...");

        let mut utxos = UTXOSet::new(storage);
        info!("create utxos...");

        //TODO: should be removed
        //======================================================================
        info!("start create genesis block...");
        // create genesis block with the genesis account
        bc.create_genesis_block(addr.as_str()).await;

        // update utxo
        utxos.reindex(&bc).await?;
        //======================================================================
        info!("Everything is ok, you have created you first bitmint chain!");

        // create a new Node
        Ok(Self {
            bc,
            utxos,
            msg_receiver: Arc::new(Mutex::new(msg_receiver)),
            swarm: Arc::new(Mutex::new(
                create_swarm(vec![BLOCK_TOPIC.clone(), TRANX_TOPIC.clone()], msg_sender)
                    .await
                    .unwrap(),
            )),
        })
    }

    /*     pub async fn list_peers(&mut self) -> Result<Vec<&PeerId>> {
        let nodes = self.swarm.lock().await.as_mut().unwrap().behaviour().mdns.discovered_nodes();
        let peers = nodes.collect::<Vec<_>>();
        Ok(peers)
    } */

    async fn sync(&mut self) -> Result<()> {
        let version = Messages::Version {
            best_height: self.bc.get_height().await,
            from_addr: PEER_ID.to_string(),
        };

        let line = serde_json::to_vec(&version)?;

        let swarm_arc = self.swarm.clone();
        let mut swarm_lock = swarm_arc.lock().await;
        let mut swarm = swarm_lock.deref_mut();
        swarm
            .behaviour_mut()
            .gossipsub
            .publish(BLOCK_TOPIC.clone(), line)
            .unwrap();

        Ok(())
    }

    async fn process_version_msg(&mut self, best_height: usize, from_addr: String) -> Result<()> {
        if self.bc.get_height().await > best_height {
            let blocks = Messages::Blocks {
                blocks: self.bc.get_blocks().await,
                height: self.bc.get_height().await,
                to_addr: from_addr,
            };
            let msg = serde_json::to_vec(&blocks)?;

            let swarm_arc = self.swarm.clone();
            let mut swarm_lock = swarm_arc.lock().await;
            let mut swarm = swarm_lock.deref_mut();

            swarm
                .behaviour_mut()
                .gossipsub
                .publish(BLOCK_TOPIC.clone(), msg)
                .unwrap();
        }
        Ok(())
    }

    async fn process_blocks_msg(
        &mut self,
        blocks: Vec<Block>,
        to_addr: String,
        height: usize,
    ) -> Result<()> {
        if PEER_ID.to_string() == to_addr && self.bc.get_height().await < height {
            for block in blocks {
                self.bc.add_block(block).await?;
            }

            self.utxos.reindex(&self.bc).await.unwrap();
        }
        Ok(())
    }

    pub async fn process_block_msg(&mut self, block: Block) -> Result<()> {
        self.bc.add_block(block).await?;
        self.utxos.reindex(&self.bc).await.unwrap();
        Ok(())
    }

    pub async fn create_wallet(&mut self, wallet_name: String) -> Result<()> {
        WALLET_MAP
            .lock()
            .await
            .entry(wallet_name.clone())
            .or_insert_with(|| {
                let mut wallets = Wallets::new().unwrap();
                let addr = wallets.create_wallet();
                info!("{}'s address is {}", wallet_name, addr);
                addr
            });
        Ok(())
    }

    pub async fn start<U: Storage + std::clone::Clone>(
        &mut self,
        matches: &ArgMatches<'_>,
        abci_server_address: SocketAddr,
    ) -> Result<()> {
        info!("开始启动节点...");

        // generate state
        let committed_state: Arc<Mutex<NodeState<T>>> = Arc::new(Mutex::new(self.clone()));
        let current_state: Arc<Mutex<Option<NodeState<T>>>> =
            Arc::new(Mutex::new(Some(self.clone())));

        // build connection
        let consensus = ConsensusConnection::new(committed_state.clone(), current_state);
        let mempool = MempoolConnection;
        let info = InfoConnection::new(committed_state);
        let snapshot = SnapshotConnection;

        // create a abci server
        let server = Server::new(consensus, mempool, info, snapshot);

        // run abci server
        spawn(async move {
            server.run(abci_server_address).await.unwrap();
        });

        // swarm info
        let mut swarm_lock = self.swarm.lock().await;
        let mut swarm = swarm_lock.deref_mut();

        // 这边如果是0.0.0.0/tpc/0的话，会监听两个tcp地址：一个是本地127.0.0.1/tpc/xxx，另一个是局域网192.168.xx.xx/tcp/xxx
        swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;

        let swarm_arc = self.swarm.clone();
        drop(swarm_lock);
        loop {
            //TODO: sync peer data
            // self.sync().await?;
            let mut swarm_lock = swarm_arc.lock().await;
            let swarm = &mut *swarm_lock;
            let msg_receiver_arc = self.msg_receiver.clone();
            tokio::select! {
                messages = async move{
                    let mut msg_receiver_lock = msg_receiver_arc.lock().await;
                    let msg_receiver = &mut *msg_receiver_lock;
                    msg_receiver.recv().await
                } => {
                    if let Some(msg) = messages {
                        match msg {
                            Messages::Version{best_height, from_addr} => {
                                self.clone().process_version_msg(best_height, from_addr).await?;
                            },
                            Messages::Blocks{blocks, to_addr, height} => {
                                self.clone().process_blocks_msg(blocks, to_addr, height).await?;
                            },
                            Messages::Block{block} => {
                                self.clone().process_block_msg(block).await?;
                            }
                        }
                    }
                },
                event = swarm_lock.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr{address, ..} => {
                            info!("Listening on {:?}", address);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("Connected to {:?}", peer_id);
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            info!("Disconnected from {:?}", peer_id);
                        }
                        SwarmEvent::ListenerClosed { addresses, .. } => {
                            info!("Listener closed for addresses: {:?}", addresses);
                        }
                        SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error } => {
                            info!("Incoming connection error (local: {:?}, send back: {:?}): {:?}", local_addr, send_back_addr, error);
                        }
                        _ => {
                            // 其他事件类型，根据需要添加
                            info!("Unhandled event: {:?}", event);
                        }
                    }
                }
            }
        }
    }
}
