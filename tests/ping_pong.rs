use luci::{
    execution::Executable,
    marshalling::{MarshallingRegistry, Regular},
    scenario::{RequiredToBe, Scenario},
};
use serde_json::json;

pub mod proto {
    //! An actor sends a [`Bro`] via the routing upon its start.
    //!
    //! Whoever receives a [`Bro`], replies with a directed [`Bro`] to
    //! the sender of the received message, unless the sender is in the list of known-peers.
    //!
    //! After sending a directed [`Bro`] the destination address is put into the list of known-peers.
    //!
    //! Once every [`TIMEOUT`] an actor sends a [`Ping`] to each of the known-peers,
    //! and marks them as a removal-candidate.
    //!
    //! If an actor receives a [`Pong`] from a known peer,
    //! that peer is no longer considered a removal-candidate.
    //!
    //! Once every [`TIMEOUT`] an actor forgets all the peers known as removal-candidates,
    //! those peers are no longer in the list of known-peers.
    //!

    use std::time::Duration;

    use elfo::message;

    pub const TIMEOUT: Duration = Duration::from_secs(10);

    #[message]
    pub struct Bro;

    #[message]
    pub struct Bye;

    #[message]
    pub struct Ping {
        pub req_id: u8,
    }

    #[message]
    pub struct Pong {
        pub req_id: u8,
    }

    #[message]
    pub struct Tick;
}

pub mod pinger {
    use std::collections::HashSet;

    use elfo::{msg, ActorGroup, Blueprint, Context};
    use tracing::{info, warn};

    use crate::proto;

    pub async fn actor(mut ctx: Context) {
        info!("ping client started");

        ctx.send(proto::Bro).await.expect("send-hello");
        ctx.attach(elfo::stream::Stream::generate(|mut emitter| async move {
            loop {
                info!("TICK: before sleep");
                tokio::time::sleep(proto::TIMEOUT).await;
                info!("TICK: after sleep, before emit");
                emitter.emit(proto::Tick).await;
                info!("TICK: after emit");
            }
        }));

        let mut req_id = 1;
        let mut peers = HashSet::new();
        let mut eviction_candidates = HashSet::new();

        while let Some(envelope) = ctx.recv().await {
            let sender = envelope.sender();
            msg!(match envelope {
                proto::Tick => {
                    for gone in eviction_candidates.drain() {
                        info!("considered gone {}", gone);
                        let _ = ctx.send_to(gone, proto::Bye).await;
                        peers.remove(&gone);
                    }
                    for survivor in peers.iter().copied() {
                        eviction_candidates.insert(survivor);
                        let _ = ctx.send_to(survivor, proto::Ping { req_id }).await;
                        req_id = req_id.wrapping_add(1);
                    }
                }
                proto::Bro => {
                    if peers.insert(sender) {
                        info!("BRO! {}", sender);
                        if let Err(reason) = ctx.send_to(sender, proto::Bro).await {
                            warn!("error while bro-eing back {}: {}", sender, reason);
                        }
                    }
                }
                proto::Ping { req_id } => {
                    info!("replying to a ping #{} from {}", req_id, sender);
                    let _ = ctx.send_to(sender, proto::Pong { req_id });
                }
                proto::Pong => {
                    info!("received a pong");
                    eviction_candidates.remove(&sender);
                }
            })
        }

        info!("bye!");
    }

    pub fn blueprint() -> Blueprint {
        ActorGroup::new().exec(actor)
    }
}

#[tokio::test]
async fn test_no_peers() {
    run_scenario(include_str!("ping_pong/test-no-peers.yaml")).await
}

#[tokio::test]
async fn test_one_peer() {
    run_scenario(include_str!("ping_pong/test-one-peer.yaml")).await
}

async fn run_scenario(scenario_text: &str) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    tokio::time::pause();

    let marshalling = MarshallingRegistry::new()
        .with(Regular::<crate::proto::Bro>)
        .with(Regular::<crate::proto::Ping>)
        .with(Regular::<crate::proto::Pong>)
        .with(Regular::<crate::proto::Bye>);
    let scenario: Scenario = serde_yaml::from_str(scenario_text).unwrap();
    let exec_graph = Executable::build(&scenario, Some(&marshalling)).expect("building graph");
    let report = exec_graph
        .start(pinger::blueprint(), json!(null))
        .await
        .run(marshalling)
        .await
        .expect("runner.run");

    let mut okay = true;
    for (event_name, required) in report
        .reached
        .into_iter()
        .filter(|(_, r)| matches!(r, RequiredToBe::Unreached))
        .chain(
            report
                .unreached
                .into_iter()
                .filter(|(_, r)| matches!(r, RequiredToBe::Reached)),
        )
    {
        okay = false;
        let opposite = match required {
            RequiredToBe::Reached => "unreached",
            RequiredToBe::Unreached => "reached",
        };
        eprintln!("- {}: {}", event_name, opposite);
    }
    assert!(okay);
}
