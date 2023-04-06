use crate::{
    blocks::Blockchain, Block, BlockchainBehaviour, Commands, Messages, SledDb, Storage,
    Transaction, UTXOSet, Wallets,
};
use anyhow::{Error, Result};
use futures::{StreamExt, task::AtomicWaker};
use libp2p::{swarm::SwarmEvent, PeerId, Swarm};
use std::io;
use std::sync::Arc;
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    sync::{mpsc, Mutex},
};
use std::ops::DerefMut;
use tracing::{error, info};

use super::{create_swarm, BLOCK_TOPIC, PEER_ID, TRANX_TOPIC, WALLET_MAP};
use clap::{crate_name, crate_version, App, AppSettings, ArgMatches, SubCommand};

// Node主要处理p2p网络同步
#[derive(Clone)]
pub struct NodeState<T = SledDb> {
    pub bc: Blockchain<T>,
    pub utxos: UTXOSet<T>,
    /*     pub msg_receiver: mpsc::UnboundedReceiver<Messages>,
    pub swarm: Swarm<BlockchainBehaviour> */
    pub msg_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Messages>>>,
    pub swarm: Arc<Mutex<Swarm<BlockchainBehaviour>>>,
}

/*

我现在有一个结构
pub struct NodeState<T = SledDb> {
    pub app: AppState,
    pub msg_receiver: mpsc::UnboundedReceiver<Messages>,
    pub swarm: Swarm<BlockchainBehaviour>,
}

pub struct AppState {
    pub bc: Blockchain<T>,
    pub utxos: UTXOSet<T>,
}

一开始，我会启动这个NodeState实例，然后在每次更新APpState中的bc的时候，调用swarm的方法进行广播，这种要怎么做呢？

*/

impl<T: Storage> NodeState<T> {
    pub async fn new(storage: Arc<T>, genesis_account: &str) -> Result<Self> {
        let (msg_sender, msg_receiver) = mpsc::unbounded_channel();

        let walle_name = String::from(genesis_account);
        let mut wallet_app = WALLET_MAP.lock().await;

        // 新建节点的时候，就创建账户，并将其塞进创世区块
        let addr = wallet_app.entry(walle_name.clone()).or_insert_with(|| {
            let mut wallets = Wallets::new().unwrap();
            let addr = wallets.create_wallet();
            info!("{}'s address is {}", walle_name, addr);
            addr
        });

        let mut bc = Blockchain::new(storage.clone()).await;
        let mut utxos = UTXOSet::new(storage);

        // create genesis block with the genesis account
        bc.create_genesis_block(addr.as_str());
        // update utxo
        utxos.reindex(&bc).await?;

        // create a new Node
        Ok(Self {
            bc,
            utxos,
            msg_receiver: Arc::new(Mutex::new(msg_receiver)),
            swarm: Arc::new(Mutex::new(
                create_swarm(vec![BLOCK_TOPIC.clone(), TRANX_TOPIC.clone()], msg_sender).await?,
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
            best_height: self.bc.get_height(),
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

    async fn transfer(&mut self, from: &str, to: &str, amount: i32) -> Result<()> {
        // 这一部分应该是在deliver_tx
        let tx = Transaction::new_utxo(from, to, amount, &self.utxos, &self.bc).await;
        let txs = vec![tx];
        let block = self.bc.mine_block(&txs).await;
        self.utxos.reindex(&self.bc).await.unwrap();

        let b = Messages::Block { block };
        let line = serde_json::to_vec(&b)?;

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
        if self.bc.get_height() > best_height {
            let blocks = Messages::Blocks {
                blocks: self.bc.get_blocks().await,
                height: self.bc.get_height(),
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
        if PEER_ID.to_string() == to_addr && self.bc.get_height() < height {
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

    pub async fn start(&mut self, matches: &ArgMatches<'_>) -> Result<()> {
        let swarm_arc = self.swarm.clone();
        let mut swarm_lock = swarm_arc.lock().await;
        let mut swarm = swarm_lock.deref_mut();
        swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;
        loop {
            let msg_receiver_arc = self.msg_receiver.clone();
            let mut msg_receiver_lock = msg_receiver_arc.lock().await;
            let mut msg_receiver = msg_receiver_lock.deref_mut();

            let swarm_arc = self.swarm.clone();
            let mut swarm_lock = swarm_arc.lock().await;
            let mut swarm = swarm_lock.deref_mut();

            self.sync().await;
            tokio::select! {
                messages = msg_receiver.recv() => {
                    if let Some(msg) = messages {
                        match msg {
                            Messages::Version{best_height, from_addr} => {
                                self.process_version_msg(best_height, from_addr).await?;
                            },
                            Messages::Blocks{blocks, to_addr, height} => {
                                self.process_blocks_msg(blocks, to_addr, height).await?;
                            },
                            Messages::Block{block} => {
                                self.process_block_msg(block).await?;
                            }
                        }
                    }
                },
                event = swarm.select_next_some() => {
                    if let SwarmEvent::NewListenAddr { address, .. } = event {
                        println!("Listening on {:?}", address);
                    }
                }
            }
        }
    }
}
