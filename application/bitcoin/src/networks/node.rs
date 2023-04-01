use crate::{
    blocks::Blockchain, Block, BlockchainBehaviour, Commands, Messages, SledDb, Storage,
    Transaction, UTXOSet, Wallets,
};
use anyhow::{Result, Error};
use futures::StreamExt;
use libp2p::{swarm::SwarmEvent, PeerId, Swarm};
use std::sync::Arc;
use std::io;
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    sync::mpsc,
};
use tracing::{error, info};

use super::{create_swarm, BLOCK_TOPIC, PEER_ID, TRANX_TOPIC, WALLET_MAP};
use clap::{crate_name, crate_version, App, AppSettings, ArgMatches, SubCommand};

pub struct Node<T = SledDb> {
    bc: Blockchain<T>,
    utxos: UTXOSet<T>,
    msg_receiver: mpsc::UnboundedReceiver<Messages>,
    swarm: Swarm<BlockchainBehaviour>,
}

impl<T: Storage> Node<T> {
    pub async fn new(storage: Arc<T>) -> Result<Self> {
        let (msg_sender, msg_receiver) = mpsc::unbounded_channel();

        Ok(Self {
            bc: Blockchain::new(storage.clone()),
            utxos: UTXOSet::new(storage),
            msg_receiver,
            swarm: create_swarm(vec![BLOCK_TOPIC.clone(), TRANX_TOPIC.clone()], msg_sender).await?,
        })
    }

    pub async fn list_peers(&mut self) -> Result<Vec<&PeerId>> {
        let nodes = self.swarm.behaviour().mdns.discovered_nodes();
        let peers = nodes.collect::<Vec<_>>();
        Ok(peers)
    }

    async fn sync(&mut self) -> Result<()> {
        let version = Messages::Version {
            best_height: self.bc.get_height(),
            from_addr: PEER_ID.to_string(),
        };

        let line = serde_json::to_vec(&version)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(BLOCK_TOPIC.clone(), line)
            .unwrap();

        Ok(())
    }

    async fn mine_block(&mut self, from: &str, to: &str, amount: i32) -> Result<()> {
        let tx = Transaction::new_utxo(from, to, amount, &self.utxos, &self.bc);
        let txs = vec![tx];
        let block = self.bc.mine_block(&txs);
        self.utxos.reindex(&self.bc).unwrap();

        let b = Messages::Block { block };
        let line = serde_json::to_vec(&b)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(BLOCK_TOPIC.clone(), line)
            .unwrap();
        Ok(())
    }

    async fn process_version_msg(&mut self, best_height: usize, from_addr: String) -> Result<()> {
        if self.bc.get_height() > best_height {
            let blocks = Messages::Blocks {
                blocks: self.bc.get_blocks(),
                height: self.bc.get_height(),
                to_addr: from_addr,
            };
            let msg = serde_json::to_vec(&blocks)?;
            self.swarm
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
                self.bc.add_block(block)?;
            }

            self.utxos.reindex(&self.bc).unwrap();
        }
        Ok(())
    }

    pub async fn process_block_msg(&mut self, block: Block) -> Result<()> {
        self.bc.add_block(block)?;
        self.utxos.reindex(&self.bc).unwrap();
        Ok(())
    }

    pub async fn create_wallet(&mut self, wallet_name: String) -> Result<()> {
        WALLET_MAP.lock().await.entry(wallet_name.clone()).or_insert_with(|| {
            let mut wallets = Wallets::new().unwrap();
            let addr = wallets.create_wallet();
            info!("{}'s address is {}", wallet_name, addr);
            addr
        });
        Ok(())
    }

    pub async fn transfer_tx(&mut self, from: String, to: String, amount: String) -> Result<()> {
        self.mine_block(&from, &to, amount.parse::<i32>().unwrap()).await?;
        Ok(())
    }

    pub async fn start(&mut self, matches: &ArgMatches<'_>) -> Result<()> {
        self.swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;
        loop {
            self.sync().await;
            tokio::select! {
                messages = self.msg_receiver.recv() => {
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
                event = self.swarm.select_next_some() => {
                    if let SwarmEvent::NewListenAddr { address, .. } = event {
                        println!("Listening on {:?}", address);
                    }
                }
            }
        }
    }
}