use futures::{executor::block_on, StreamExt};
use libp2p::{
    autonat,
    core::{transport::OrTransport, upgrade},
    dcutr::{
        self,
        behaviour::{Behaviour as DcutrBehaviour, Event as DcutrEvent},
    },
    dns::DnsConfig,
    gossipsub::{
        self, error::PublishError, Gossipsub, GossipsubEvent, IdentTopic as Topic,
        MessageAuthenticity, ValidationMode,
    },
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity::{self, Keypair},
    mdns::{Mdns, MdnsEvent},
    mplex::MplexConfig,
    multiaddr::Protocol,
    noise,
    relay::v2::client::{self, Client as RelayClient, Event as RelayEvent},
    swarm::{NetworkBehaviourEventProcess, SwarmBuilder, SwarmEvent},
    tcp::TcpConfig,
    Multiaddr, NetworkBehaviour, PeerId, Transport,
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

    pub relay_server_address: Option<Multiaddr>,
    pub relay_server_peer_id: Option<PeerId>,
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

    let (relay_transport, relay_client) = RelayClient::new_transport_and_behaviour(local_peer_id);

    let transport = OrTransport::new(
        block_on(DnsConfig::system(TcpConfig::new().port_reuse(true))).unwrap(),
        relay_transport,
    )
    .upgrade(upgrade::Version::V1)
    .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
    .multiplex(MplexConfig::new())
    .boxed();

    let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .max_transmit_size(1024 * 1024 * 50)
        .build()
        .expect("Valid config");

    let gossipsub = gossipsub::Gossipsub::new(
        MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )
    .expect("Correct configuration");

    let mut swarm = {
        let mut behaviour = Behaviour {
            gossipsub,
            mdns: Mdns::new(Default::default()).await.unwrap(),
            identify: Identify::new(IdentifyConfig::new(
                "/uniclip/0.1.0".into(),
                local_key.public(),
            )),
            auto_nat: autonat::Behaviour::new(
                local_peer_id,
                autonat::Config {
                    retry_interval: Duration::from_secs(10),
                    refresh_interval: Duration::from_secs(30),
                    boot_delay: Duration::from_secs(5),
                    throttle_server_period: Duration::ZERO,
                    ..Default::default()
                },
            ),

            relay_client,
            dcutr: DcutrBehaviour::new(),

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

    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
        .unwrap();

    // connect relay
    {
        if let Some(relay_server_peer_id) = config.relay_server_peer_id {
            swarm
                .behaviour_mut()
                .auto_nat
                .add_server(relay_server_peer_id, config.relay_server_address.clone());

            if let Some(relay_server_address) = config.relay_server_address {
                swarm
                    .listen_on(relay_server_address.with(Protocol::P2pCircuit))
                    .unwrap();
            }
        }

        // my dev server
        let dev_relay_address: Multiaddr = "/ip4/42.193.117.213/tcp/34567".parse().unwrap();
        let dev_relay_address_p2p: Multiaddr = "/ip4/42.193.117.213/tcp/34567/p2p/12D3KooWNoSoxPRWovwRFnheDwrgo6cufbYGtWSrfKXVhSDxTzSV".parse().unwrap();
        swarm.behaviour_mut().auto_nat.add_server(
            "12D3KooWNoSoxPRWovwRFnheDwrgo6cufbYGtWSrfKXVhSDxTzSV"
                .parse()
                .unwrap(),
            Some(dev_relay_address.clone()),
        );
        swarm
            .listen_on(dev_relay_address_p2p.with(Protocol::P2pCircuit))
            .unwrap();
    }

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

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
struct Behaviour {
    #[behaviour(event_process = true)]
    gossipsub: Gossipsub,
    #[behaviour(event_process = true)]
    mdns: Mdns,
    #[behaviour(event_process = false)]
    identify: Identify,
    #[behaviour(event_process = false)]
    auto_nat: autonat::Behaviour,

    relay_client: RelayClient,
    dcutr: DcutrBehaviour,

    #[behaviour(ignore)]
    from_net_tx: Sender<ClipMsg>,
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for Behaviour {
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

impl NetworkBehaviourEventProcess<MdnsEvent> for Behaviour {
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

#[derive(Debug)]
enum Event {
    AutoNat(autonat::Event),
    Identify(IdentifyEvent),
    Mdns(MdnsEvent),
    Gossipsub(GossipsubEvent),
    Relay(RelayEvent),
    Dcutr(DcutrEvent),
}

impl From<autonat::Event> for Event {
    fn from(v: autonat::Event) -> Self {
        Self::AutoNat(v)
    }
}

impl From<IdentifyEvent> for Event {
    fn from(v: IdentifyEvent) -> Self {
        Self::Identify(v)
    }
}

impl From<MdnsEvent> for Event {
    fn from(v: MdnsEvent) -> Self {
        Self::Mdns(v)
    }
}

impl From<GossipsubEvent> for Event {
    fn from(v: GossipsubEvent) -> Self {
        Self::Gossipsub(v)
    }
}

impl From<client::Event> for Event {
    fn from(e: client::Event) -> Self {
        Event::Relay(e)
    }
}

impl From<dcutr::behaviour::Event> for Event {
    fn from(e: dcutr::behaviour::Event) -> Self {
        Event::Dcutr(e)
    }
}
