use futures::StreamExt;
use libp2p::{
    core::upgrade,
    gossipsub::{
        self, error::PublishError, Gossipsub, GossipsubEvent, IdentTopic as Topic,
        MessageAuthenticity, ValidationMode,
    },
    identity::{self, Keypair},
    mdns::{Mdns, MdnsEvent},
    mplex, noise,
    swarm::{NetworkBehaviourEventProcess, SwarmBuilder, SwarmEvent},
    tcp::TokioTcpConfig,
    NetworkBehaviour, PeerId, Transport,
};
use prost::Message;
use std::{
    fs,
    io::{Read, Write},
    path,
    sync::mpsc::Sender,
    time::Duration,
};
use tokio::sync::mpsc::Receiver;
use uniclip_proto::ClipMsg;

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct MyBehaviour {
    gossipsub: Gossipsub,
    mdns: Mdns,

    #[behaviour(ignore)]
    from_net_tx: Sender<ClipMsg>,
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for MyBehaviour {
    fn inject_event(&mut self, message: GossipsubEvent) {
        if let GossipsubEvent::Message {
            propagation_source: _,
            message_id: _,
            message,
        } = message
        {
            if let Ok(clip_msg) = ClipMsg::decode(message.data.as_slice()) {
                self.from_net_tx.send(clip_msg).unwrap();
            }
        }
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for MyBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer, addr) in list {
                    println!("new peer: {peer} - {addr}");
                    self.gossipsub.add_explicit_peer(&peer);
                }
            }
            MdnsEvent::Expired(list) => {
                for (peer, _) in list {
                    if !self.mdns.has_node(&peer) {
                        self.gossipsub.remove_explicit_peer(&peer);
                    }
                }
            }
        }
    }
}

pub fn get_local_keypair_peerid(config: &Config) -> (Keypair, PeerId) {
    let filepath = path::Path::new(&config.dir).join("keypair");

    let keypair = match fs::File::open(&filepath) {
        Ok(mut file) => {
            let mut buffer = vec![0; file.metadata().unwrap().len() as usize];
            file.read(&mut buffer).expect("buffer overflow");
            let keypair = Keypair::from_protobuf_encoding(&buffer).unwrap();
            keypair
        }
        Err(_) => {
            let keypair = identity::Keypair::generate_ed25519();
            let buffer = keypair.to_protobuf_encoding().unwrap();
            if let Ok(mut file) = fs::File::create(&filepath) {
                file.write(&buffer).unwrap();
            }
            keypair
        }
    };

    let peer_id = PeerId::from(keypair.public());
    (keypair, peer_id)
}

pub struct Config {
    pub dir: String,
    pub topic: String,
}

pub async fn trans(
    config: Config,
    from_net_tx: Sender<ClipMsg>,
    to_net_rx: Receiver<ClipMsg>,
) -> ! {
    let (local_key, local_peer_id) = get_local_keypair_peerid(&config);
    println!("Local peer id: {:?}", local_peer_id);

    let topic = Topic::new(config.topic);

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&local_key)
        .expect("Signing libp2p-noise static DH keypair failed.");

    let transport = TokioTcpConfig::new()
        .nodelay(true)
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .max_transmit_size(1024 * 1024 * 50)
        .build()
        .expect("Valid config");

    let gossipsub =
        gossipsub::Gossipsub::new(MessageAuthenticity::Signed(local_key), gossipsub_config)
            .expect("Correct configuration");

    let mut swarm = {
        let mdns = Mdns::new(Default::default()).await.unwrap();
        let mut behaviour = MyBehaviour {
            gossipsub,
            mdns,
            from_net_tx,
        };

        behaviour.gossipsub.subscribe(&topic).unwrap();

        SwarmBuilder::new(transport, behaviour, local_peer_id)
            // We want the connection background tasks to be spawned
            // onto the tokio runtime.
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build()
    };

    // Listen on all interfaces and whatever port the OS assigns
    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    // Kick it off
    let mut to_net_rx = to_net_rx;
    loop {
        tokio::select! {
            clip_msg = to_net_rx.recv() => {
                if let Some(clip_msg) = clip_msg {
                    match swarm.behaviour_mut().gossipsub.publish(topic.clone(), clip_msg.encode_to_vec()) {
                        Ok(_) => {},
                        Err(err) => {
                            match err {
                                PublishError::InsufficientPeers => {},
                                _ => {panic!("{err}");}
                            }
                        },
                    }
                }
            }

            event = swarm.select_next_some() => if let SwarmEvent::NewListenAddr { address, .. } =  event {
                println!("Listening on {:?}", address);
            }
        }
    }
}
