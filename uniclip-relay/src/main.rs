use clap::Parser;
use futures::{executor::block_on, stream::StreamExt};
use libp2p::{
    autonat,
    core::upgrade,
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity::Keypair,
    mplex,
    multiaddr::Protocol,
    noise,
    ping::{Ping, PingConfig, PingEvent},
    relay::v2::relay::{self, Relay},
    swarm::{Swarm, SwarmEvent},
    tcp::TokioTcpConfig,
    Multiaddr, NetworkBehaviour, PeerId, Transport,
};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, Ipv6Addr},
};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let opt = Opt::parse();
    println!("opt: {:?}", opt);

    let (local_key, local_peer_id) = get_local_keypair_peerid("keypath");
    println!("Local peer id: {:?}", local_peer_id);

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&local_key)
        .expect("Signing libp2p-noise static DH keypair failed.");

    let transport = TokioTcpConfig::new()
        .nodelay(true)
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let behaviour = Behaviour::new(local_key, local_peer_id);
    let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

    // Listen on all interfaces
    let listen_addr = Multiaddr::empty()
        .with(match opt.use_ipv6 {
            Some(true) => Protocol::from(Ipv6Addr::UNSPECIFIED),
            _ => Protocol::from(Ipv4Addr::UNSPECIFIED),
        })
        .with(Protocol::Tcp(opt.port));
    swarm.listen_on(listen_addr)?;

    block_on(async {
        loop {
            match swarm.next().await.expect("Infinite Stream.") {
                SwarmEvent::Behaviour(Event::Relay(event)) => {
                    println!("{:?}", event)
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {:?}", address);
                }
                _ => {}
            }
        }
    })
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event", event_process = false)]
struct Behaviour {
    relay: Relay,
    ping: Ping,
    identify: Identify,
    auto_nat: autonat::Behaviour,
}

impl Behaviour {
    fn new(local_key: Keypair, local_peer_id: PeerId) -> Self {
        Behaviour {
            relay: Relay::new(local_peer_id, Default::default()),
            ping: Ping::new(PingConfig::new()),
            identify: Identify::new(IdentifyConfig::new(
                "/ipfs/0.1.0".to_string(),
                local_key.public(),
            )),
            auto_nat: autonat::Behaviour::new(local_peer_id, autonat::Config::default()),
        }
    }
}

#[derive(Debug)]
enum Event {
    AutoNat(autonat::Event),
    Ping(PingEvent),
    Identify(IdentifyEvent),
    Relay(relay::Event),
}

impl From<PingEvent> for Event {
    fn from(e: PingEvent) -> Self {
        Event::Ping(e)
    }
}

impl From<IdentifyEvent> for Event {
    fn from(e: IdentifyEvent) -> Self {
        Event::Identify(e)
    }
}

impl From<relay::Event> for Event {
    fn from(e: relay::Event) -> Self {
        Event::Relay(e)
    }
}

impl From<autonat::Event> for Event {
    fn from(v: autonat::Event) -> Self {
        Self::AutoNat(v)
    }
}

pub fn get_local_keypair_peerid(key_path: &str) -> (Keypair, PeerId) {
    let keypair = match fs::File::open(key_path) {
        Ok(mut file) => {
            let mut buffer = vec![0; file.metadata().unwrap().len() as usize];
            file.read(&mut buffer).expect("buffer overflow");
            let keypair = Keypair::from_protobuf_encoding(&buffer).unwrap();
            keypair
        }
        Err(_) => {
            let keypair = Keypair::generate_ed25519();
            let buffer = keypair.to_protobuf_encoding().unwrap();
            if let Ok(mut file) = fs::File::create(key_path) {
                file.write(&buffer).unwrap();
            }
            keypair
        }
    };

    let peer_id = PeerId::from(keypair.public());
    (keypair, peer_id)
}

#[derive(Debug, Parser)]
#[clap(name = "libp2p relay")]
struct Opt {
    /// Determine if the relay listen on ipv6 or ipv4 loopback address. the default is ipv4
    #[clap(long)]
    use_ipv6: Option<bool>,

    /// The port used to listen on all interfaces
    #[clap(long)]
    port: u16,
}
