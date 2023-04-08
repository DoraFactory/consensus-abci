mod behaviour;
mod node;
mod command;

pub use behaviour::*;
pub use node::*;
pub use command::*;
use tracing::info;

use once_cell::sync::Lazy;
use tokio::sync::{mpsc, Mutex};
use std::{
    time::Duration, 
    collections::{hash_map::DefaultHasher, HashMap}, 
    hash::{Hasher, Hash}, sync::Arc,
};
use anyhow::Result;
use libp2p::{
    gossipsub::{IdentTopic as Topic, GossipsubConfigBuilder, ValidationMode, MessageId, GossipsubMessage}, 
    swarm::SwarmBuilder, 
    identity::Keypair, 
    tcp::{GenTcpConfig, tokio::Tcp},
    core::upgrade, 
    Swarm, PeerId, noise, yamux, Transport,
};

pub type TokioTcpConfig = GenTcpConfig<Tcp>;


pub static ID_KEYS: Lazy<Keypair> = Lazy::new(Keypair::generate_ed25519);
pub static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(ID_KEYS.public()));
pub static BLOCK_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("blocks"));
pub static TRANX_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("tranxs"));

static WALLET_MAP: Lazy<Arc<Mutex<HashMap<String, String>>>> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

async fn create_swarm(topics: Vec<Topic>, msg_sender: mpsc::UnboundedSender<Messages>) -> Result<Swarm<BlockchainBehaviour>> {
    info!("Your local peer id: {:?}", *PEER_ID);

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new().into_authentic(&ID_KEYS)?;

    let transport = TokioTcpConfig::new()
        .nodelay(true)
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(yamux::YamuxConfig::default())
        .boxed();

    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
    };

    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()
        .expect("Valid config");

    let mut behaviour = BlockchainBehaviour::new(ID_KEYS.clone(), gossipsub_config, msg_sender).await?;
    for topic in topics.iter() {
        behaviour.gossipsub.subscribe(topic).unwrap();
    }

    //TODO: 这里生成的类型和之前不一样，有点问题？？？？
    let swarm = SwarmBuilder::new(transport, behaviour, PEER_ID.clone())
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        })).build();
    
    println!("Exiting create_swarm...");
    Ok(swarm)
}