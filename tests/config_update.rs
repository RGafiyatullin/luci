use elfo::config::AnyConfig;
use elfo::messages::UpdateConfig;
use elfo::AnyMessage;
use luci::execution::{Executable, SourceCodeLoader};
use luci::marshalling::{Injected, MarshallingRegistry, Regular};
use serde_json::json;

mod proto {
    use elfo::message;

    #[message]
    pub struct Ping;

    #[message]
    pub struct Pong {
        pub value: u64,
    }
}

mod configurable {
    use elfo::{msg, ActorGroup, Blueprint, Context};
    use tracing::info;

    use crate::proto;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Config {
        value: u64,
    }

    pub fn blueprint() -> Blueprint {
        ActorGroup::new().config::<Config>().exec(actor)
    }

    pub async fn actor(mut ctx: Context<Config>) {
        loop {
            let Some(envelope) = ctx.recv().await else {
                break
            };
            let sender = envelope.sender();
            let &Config { value } = ctx.config();

            msg!(match envelope {
                proto::Ping => {
                    let _ = ctx.send_to(sender, proto::Pong { value }).await;
                },
            })
        }
        info!("Bye!")
    }
}

#[tokio::test]
async fn config_update() {
    let scenario_file = "tests/config_update/scenario.luci.yaml";
    let config_0 = json!({
        "value": 1,
    });
    let config_1 = json!({
        "value": 2,
    });

    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    tokio::time::pause();

    let marshalling = MarshallingRegistry::new()
        .with(Regular::<crate::proto::Ping>)
        .with(Regular::<crate::proto::Pong>)
        .with(Regular::<elfo::messages::UpdateConfig>)
        .with(Injected {
            key:   "update-config".into(),
            value: AnyMessage::new({
                let serde_value = serde_json::from_value(config_1).expect("serde_value");
                let any_config = AnyConfig::from_value(serde_value);
                UpdateConfig::new(any_config)
            }),
        });

    let (key_main, sources) = SourceCodeLoader::new()
        .load(scenario_file)
        .expect("SourceLoader::load");
    let executable = Executable::build(marshalling, &sources, key_main).expect("building graph");
    let report = executable
        .start(
            configurable::blueprint(),
            config_0,
            [("$VALUE_1".into(), json!(1)), ("$VALUE_2".into(), json!(2))],
        )
        .await
        .run()
        .await
        .expect("runner.run");

    let _ = report.dump_record_log(std::io::stderr().lock(), &sources, &executable);
    assert!(report.is_ok(), "{}", report.message(&executable, &sources));
}
