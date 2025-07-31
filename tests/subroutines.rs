use luci::execution::{Executable, SourceCodeLoader};
use luci::marshalling::{MarshallingRegistry, Regular, Request};
use serde_json::json;
use test_case::test_case;

mod proto {
    pub mod smalltalk {
        use elfo::message;

        #[message(ret = NotMuch)]
        pub struct Whatsup {
            pub topic: i32,
        }

        #[message]
        pub struct NotMuch {
            pub subs_id: i32,
        }

        #[message]
        pub struct OhByTheWay {
            pub subs_id: i32,
        }

        #[message]
        pub struct NoWay {
            pub subs_id: i32,
        }
    }

    pub mod partying {
        use elfo::message;

        #[message(ret = ByAllMeans)]
        pub struct MayI;

        #[message]
        pub struct ByAllMeans;

        #[message]
        pub struct Chug;

        #[message]
        pub struct Gulp;

        #[message]
        pub struct SeeYou;
    }
}

mod socialite {
    use elfo::{msg, ActorGroup, Blueprint, Context};

    use crate::proto;

    pub fn blueprint() -> Blueprint {
        ActorGroup::new().exec(main)
    }

    async fn main(mut ctx: Context) {
        let _snapshot = ctx
            .request(proto::smalltalk::Whatsup { topic: 1 })
            .resolve()
            .await
            .expect("Subscribe");
        let _joined = ctx
            .request(proto::partying::MayI)
            .resolve()
            .await
            .expect("joining the party");

        let mut updates_left: usize = 3;

        while let Some(envelope) = ctx.recv().await {
            let sender = envelope.sender();
            msg!(match envelope {
                proto::partying::Chug => {
                    let _ = ctx.send_to(sender, proto::partying::Gulp).await;
                },
                proto::smalltalk::OhByTheWay { subs_id } => {
                    let _ = ctx
                        .send_to(sender, proto::smalltalk::NoWay { subs_id })
                        .await;
                    let Some(u) = updates_left.checked_sub(1) else {
                        break;
                    };
                    updates_left = u;
                },
            })
        }

        let _ = ctx.send(proto::partying::SeeYou).await;
    }
}

#[test_case("main.yaml", &["tests/subroutines"])]
#[tokio::test]
async fn run_scenario(scenario_file: &str, search_path: &[&str]) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_max_level(tracing::Level::TRACE)
        .try_init();
    tokio::time::pause();

    let marshalling = MarshallingRegistry::new()
        .with(Request::<crate::proto::smalltalk::Whatsup>)
        .with(Regular::<crate::proto::smalltalk::OhByTheWay>)
        .with(Regular::<crate::proto::smalltalk::NoWay>)
        .with(Request::<crate::proto::partying::MayI>)
        .with(Regular::<crate::proto::partying::SeeYou>)
        .with(Regular::<crate::proto::partying::Chug>)
        .with(Regular::<crate::proto::partying::Gulp>);

    let (key_main, sources) = SourceCodeLoader::new()
        .with_search_path(search_path)
        .load(scenario_file)
        .expect("SourceLoader::load");
    let executable = Executable::build(marshalling, &sources, key_main).expect("building graph");
    let report = executable
        .start(socialite::blueprint(), json!(null), [])
        .await
        .run()
        .await
        .expect("runner.run");

    report
        .dump_record_log(std::io::stderr().lock(), &sources, &executable)
        .expect("ew...");
    assert!(report.is_ok(), "{}", report.message());
}
