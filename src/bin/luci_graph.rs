use std::fs::{read_to_string, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use clap::Parser;
use luci::scenario::Scenario;
use luci::visualization::draw_scenario;

#[derive(Parser, Debug)]
#[command(
    name = "luci-graph",
    about = "Generate a Graphviz DOT graph from a scenario description."
)]
struct Args {
    #[clap(long = "input", short = 'i', help = "Scenario file (default: stdin)")]
    scenario_file: Option<PathBuf>,
    #[clap(long = "output", short = 'o', help = "Graphviz file (default: stdout")]
    output_file:   Option<PathBuf>,
    #[clap(
        long = "verbose",
        short = 'v',
        default_value_t = false,
        help = "Add additional information to the graph"
    )]
    verbose:       bool,
}

fn main() {
    let args = Args::parse();

    let result = run(&args);

    match args.output_file {
        Some(path) => {
            let mut file = File::create(path).expect("Failed to create output file");
            file.write(result.as_bytes())
                .expect("Failed to write to output file");
        },
        None => {
            println!("{}", result);
        },
    }
}

fn run(args: &Args) -> String {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    let scenario = if let Some(path) = &args.scenario_file {
        read_to_string(path).expect("Failed to read scenario file")
    } else {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .expect("Failed to read from stdin");
        input.trim().to_string()
    };

    let scenario: Scenario =
        serde_yaml::from_str(&scenario).expect("Failed to parse YAML scenario file");

    draw_scenario(&scenario, args.verbose)
}

#[cfg(test)]
mod test {
    use super::run;

    #[test]
    fn output_snapshot() {
        let args = super::Args {
            scenario_file: Some("tests/luci_graph/sample.yml".into()),
            output_file:   None,
            verbose:       true,
        };
        let result = run(&args);

        insta::assert_snapshot!(result);
    }
}
