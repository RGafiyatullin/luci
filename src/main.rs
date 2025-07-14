use std::{fs::read_to_string, path::PathBuf};

use clap::Parser;
use luci::{execution_graph::ExecutionGraph, messages::Messages, scenario::Scenario};

#[derive(Parser, Debug)]
struct Args {
    #[clap(help = "Path to the scenario file", short = 'i', long = "input")]
    input: PathBuf,
    #[clap(help = "Path to the output file", short = 'o', long = "output")]
    output: PathBuf,
}

fn main() {
    let args = Args::parse();

    let scenario_text = &read_to_string(args.input).expect("Failed to read scenario file");
    let scenario: Scenario =
        serde_yaml::from_str(scenario_text).expect("Failed to parse YAML scenario file");

    let messages = Messages::new();

    let execution_graph = ExecutionGraph::builder(messages)
        .build(&scenario)
        .expect("Failed to build execution graph");
}
