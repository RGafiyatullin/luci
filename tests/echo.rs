use luci::{
    execution::{Executable, SourceLoader},
    marshalling::{MarshallingRegistry, Regular, Request},
};
use serde_json::json;

pub mod proto {
    use elfo::message;
    use serde_json::Value;

    #[message]
    pub struct Hey;

    #[message]
    pub struct V(pub Value);

    #[message(ret = Value)]
    pub struct R(pub Value);
}

pub mod echo {
    use crate::proto;
    use elfo::{msg, ActorGroup, Blueprint, Context};
    use serde_json::json;

    pub async fn actor(mut ctx: Context) {
        while let Some(envelope) = ctx.recv().await {
            let sender = envelope.sender();
            msg!(match envelope {
                proto::Hey => {
                    ctx.request_to(sender, proto::R(json!("hello!")))
                        .resolve()
                        .await
                        .expect("oh :(");
                }
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
    run_scenario("tests/echo/bind-node.yaml").await;
}

#[tokio::test]
async fn marshalling() {
    run_scenario("tests/echo/marshalling.yaml").await;
}

#[tokio::test]
async fn request_response() {
    run_scenario("tests/echo/request-response.yaml").await;
}

async fn run_scenario(scenario_file: &str) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    tokio::time::pause();

    let marshalling = MarshallingRegistry::new()
        .with(Regular::<crate::proto::V>)
        .with(Request::<crate::proto::R>)
        .with(Regular::<crate::proto::Hey>);

    let (key_main, sources) = SourceLoader::new()
        .load(scenario_file)
        .expect("SourceLoader::load");
    let exec_graph = Executable::build(marshalling, &sources, key_main).expect("building graph");
    let report = exec_graph
        .start(echo::blueprint(), json!(null))
        .await
        .run()
        .await
        .expect("runner.run");

    assert!(report.is_ok(), "{}", report.message());
}
