use insta::assert_debug_snapshot;
use luci::execution::SourceCodeLoader;
use test_case::test_case;

#[test_case("00", "tests/source_loading/00-the-simplest-case.yaml", &["."])]
#[test_case("01", "tests/source_loading/01-one-inclusion.yaml", &["."])]
#[test_case("00.a", "00-the-simplest-case.yaml", &["tests/source_loading"])]
#[test_case("01.a", "01-one-inclusion.yaml", &["tests/source_loading"])]
#[test_case("02", "tests/source_loading/02-direct-cyclic-inclusion.yaml", &["."])]
#[test_case("02.a", "02-direct-cyclic-inclusion.yaml", &["tests/source_loading"])]
#[test_case("03", "03-indirect-cyclic-inclusion.yaml", &["tests/source_loading"])]
#[test_case("04", "04-diamond.yaml", &["tests/source_loading", "tests/source_loading/04-diamond"])]
fn load_sources(name: &str, main: &str, search_paths: &[&str]) {
    let mut loader = SourceCodeLoader::new();
    loader.search_path = search_paths.into_iter().copied().map(From::from).collect();
    let outcome = loader.load(main);
    assert_debug_snapshot!(name, outcome);
}
