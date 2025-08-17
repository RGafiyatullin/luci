use luci::execution::{Executable, SourceCodeLoader};
use luci::marshalling::{MarshallingRegistry, Regular};
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

    use elfo::{assert_msg, ActorGroup, Blueprint, Context};

    use crate::proto;

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

#[tokio::test]
async fn with_intervals() {
    run_scenario("tests/recv_timeout/with-intervals.yaml").await;
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
    let executable = Executable::build(marshalling, &sources, key_main).expect("building graph");
    let report = executable
        .start(echo::blueprint(), json!(null), [])
        .await
        .run()
        .await
        .expect("runner.run");

    report
        .dump_record_log(std::io::stderr().lock(), &sources, &executable)
        .unwrap();
    assert!(report.is_ok(), "{}", report.message());
}
