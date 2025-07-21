use luci::{
    execution::{Executable, SourceCodeLoader},
    marshalling::{MarshallingRegistry, Regular},
};
use serde_json::json;

pub mod proto {
    use elfo::message;

    #[message]
    pub struct Hi;

    #[message]
    pub struct Bye;
}

pub mod echo {
    use std::time::Duration;

    use crate::proto;
    use elfo::{assert_msg, ActorGroup, Blueprint, Context};
    
    pub async fn actor(mut ctx: Context) {
        let envelope = ctx.recv().await.expect("where's my Hi");
        let reply_to = envelope.sender();
        assert_msg!(envelope, proto::Hi);

        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = ctx.send_to(reply_to, proto::Hi).await;

        tokio::time::sleep(Duration::from_secs(60)).await;
        let _ = ctx.send_to(reply_to, proto::Bye).await;
    }

    pub fn blueprint() -> Blueprint {
        ActorGroup::new().exec(actor)
    }
}

#[tokio::test]
async fn no_timeouts() {
    run_scenario("tests/recv_timeout/no-timeouts.yaml").await;
}

#[tokio::test]
async fn with_timeouts() {
    run_scenario("tests/recv_timeout/with-timeouts.yaml").await;
}

async fn run_scenario(scenario_file: &str) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    tokio::time::pause();

    let marshalling = MarshallingRegistry::new()
        .with(Regular::<crate::proto::Hi>)
        .with(Regular::<crate::proto::Bye>);

    let (key_main, sources) = SourceCodeLoader::new()
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
