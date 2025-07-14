use insta::{assert_debug_snapshot, assert_yaml_snapshot};
use luci::{
    execution_graph::ExecutionGraph,
    messages::{Messages, Mock},
    scenario::Scenario,
};
use test_case::test_case;

#[test_case("01", true)]
#[test_case("02", true)]
#[test_case("03", true)]
#[test_case("04", false)]
fn run(name: &str, build_graph: bool) {
    let input = format!("tests/scenarios/{name}.yaml");
    let scenario: Scenario =
        serde_yaml::from_str(std::fs::read_to_string(input).expect("read file").as_str())
            .expect("serde::de");
    assert_debug_snapshot!(format!("{name}.debug"), scenario);
    assert_yaml_snapshot!(format!("{name}.yaml"), scenario);

    let messages = Messages::new()
        .with(Mock::msg("one::two::Three"))
        .with(Mock::req("one::two::IsAThree"))
        .with(Mock::msg("protocol_basic::Start"))
        .with(Mock::msg("protocol_basic::Started"))
        .with(Mock::msg("protocol_basic::KeepAlive"));

    if build_graph {
        let _exec_graph = ExecutionGraph::builder(messages)
            .build(&scenario)
            .expect("build exec-graph");
    }
}
