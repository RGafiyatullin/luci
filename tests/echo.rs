use luci::{
    execution_graph::ExecutionGraph,
    messages::{Messages, Regular, Request},
    scenario::Scenario,
};
use serde_json::json;

pub mod proto {
    use elfo::message;
    use serde_json::Value;

    #[message]
    pub struct V(pub Value);

    #[message(ret = Value)]
    pub struct R(pub Value);
}

pub mod echo {
    use crate::proto;
    use elfo::{msg, ActorGroup, Blueprint, Context};

    pub async fn actor(mut ctx: Context) {
        while let Some(envelope) = ctx.recv().await {
            let sender = envelope.sender();
            msg!(match envelope {
                v @ proto::V => {
                    let _ = ctx.send_to(sender, v).await;
                }
                (r @ proto::R, t) => {
                    let _ = ctx.respond(t, r.0);
                }
            })
        }
    }

    pub fn blueprint() -> Blueprint {
        ActorGroup::new().exec(actor)
    }
}

#[tokio::test]
async fn bind_node() {
    run_scenario(include_str!("echo/bind-node.yaml")).await;
}

#[tokio::test]
async fn marshalling() {
    run_scenario(include_str!("echo/marshalling.yaml")).await;
}

async fn run_scenario(scenario_text: &str) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    tokio::time::pause();

    let messages = Messages::new()
        .with(Regular::<crate::proto::V>)
        .with(Request::<crate::proto::R>);
    let scenario: Scenario = serde_yaml::from_str(scenario_text).unwrap();
    let exec_graph = ExecutionGraph::builder(messages)
        .build(&scenario)
        .expect("building graph");
    let report = exec_graph
        .make_runner(echo::blueprint(), json!(null))
        .await
        .run()
        .await
        .expect("runner.run");

    assert!(report.is_ok(), "{}", report.message());
}
