use insta::{assert_debug_snapshot, assert_yaml_snapshot};
use luci::execution::{Executable, SourceCodeLoader};
use luci::marshalling::{MarshallingRegistry, Mock};
use luci::scenario::Scenario;
use test_case::test_case;

#[test_case("01-minimal", Some(vec![]))]
#[test_case("02-with-types", Some(vec![
    ("One", true),
    ("Two", false),
]))]
#[test_case("03-with-actors", Some(vec![]))]
#[test_case("04-with-single-bind", Some(vec![]))]
#[test_case("05-with-single-send", Some(vec![("A",false)]))]
#[test_case("07-with-single-respond", None)]
#[test_case("08-with-single-delay", Some(vec![]))]
#[test_case("09-with-single-call", None)]
fn run(name: &str, build_executable_with_messages: Option<Vec<(&str, bool)>>) {
    let file = format!("tests/syntax/{name}.luci.yaml");
    let yaml = std::fs::read_to_string(&file).expect("fs::read_to_string");
    let scenario: Scenario = serde_yaml::from_str(&yaml).expect("yaml::from_str<Scenario>");

    assert_debug_snapshot!(format!("{name}-debug"), scenario);
    assert_yaml_snapshot!(format!("{name}-yaml"), scenario);

    if let Some(ms) = build_executable_with_messages {
        let mut marshalling = MarshallingRegistry::new();
        for (fqn, is_request) in ms {
            marshalling = marshalling.with(Mock::new(fqn, is_request));
        }

        let (key_main, sources) = SourceCodeLoader::new()
            .load(file)
            .expect("SourceLoader::load");

        let _executable =
            Executable::build(marshalling, &sources, key_main).expect("Executable::build");
    }
}
