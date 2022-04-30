use crate::{proto::ClipMsg, Settings};
use futures::StreamExt;
use libp2p::{
    core::upgrade,
    gossipsub::{
        self, error::PublishError, Gossipsub, GossipsubEvent, IdentTopic as Topic,
        MessageAuthenticity, ValidationMode,
    },
    identity,
    mdns::{Mdns, MdnsEvent},
    mplex, noise,
    swarm::{NetworkBehaviourEventProcess, SwarmBuilder, SwarmEvent},
    tcp::TokioTcpConfig,
    NetworkBehaviour, PeerId, Transport,
};
use prost::Message;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct MyBehaviour {
    gossipsub: Gossipsub,
    mdns: Mdns,

    #[behaviour(ignore)]
    from_net_tx: std::sync::mpsc::Sender<ClipMsg>,
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

pub async fn trans(
    settings: &Settings,
    from_net_tx: std::sync::mpsc::Sender<ClipMsg>,
    to_net_rx: Receiver<ClipMsg>,
) -> ! {
    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {:?}", local_peer_id);

    let topic = Topic::new(settings.domain.to_owned());

    // Create a keypair for authenticated encryption of the transport.
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&local_key)
        .expect("Signing libp2p-noise static DH keypair failed.");

    // Create a tokio-based TCP transport use noise for authenticated
    // encryption and Mplex for multiplexing of substreams on a TCP stream.
    let transport = TokioTcpConfig::new()
        .nodelay(true)
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
        .max_transmit_size(1024 * 1024 * 50)
        .build()
        .expect("Valid config");
    // build a gossipsub network behaviour
    let gossipsub: gossipsub::Gossipsub =
        gossipsub::Gossipsub::new(MessageAuthenticity::Signed(local_key), gossipsub_config)
            .expect("Correct configuration");

    // Create a Swarm to manage peers and events.
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
