use serde::Deserialize;
use std::{
    fs::{read_to_string, File},
    io::{Read, Write},
    path::PathBuf,
};

use clap::{Parser, ValueEnum};
use luci::{execution::Executable, graphics::RenderGraph, scenario::Scenario};

#[derive(Clone, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum GraphDrawSource {
    #[clap(name = "raw")]
    Scenario,
    #[clap(name = "graph")]
    Executable,
}

#[derive(Parser, Debug)]
#[command(
    name = "luci",
    about = "Generate a Graphviz DOT graph from a scenario description."
)]
struct Args {
    #[clap(long = "input", short = 'i', help = "Scenario file (default: stdin)")]
    scenario_file: Option<PathBuf>,
    #[clap(long = "output", short = 'o', help = "Graphviz file (default: stdout")]
    output_file: Option<PathBuf>,
    #[clap(
        long = "source",
        short = 's',
        help = "Source of data. Raw scenario, or parsed graph"
    )]
    source: GraphDrawSource,
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let args = Args::parse();

    let scenario = match args.scenario_file {
        Some(path) => read_to_string(path).expect("Failed to read scenario file"),
        None => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .expect("Failed to read from stdin");
            input.trim().to_string()
        }
    };

    let scenario: Scenario =
        serde_yaml::from_str(&scenario).expect("Failed to parse YAML scenario file");

    let result = match args.source {
        GraphDrawSource::Scenario => scenario.render(),
        GraphDrawSource::Executable => {
            let executable =
                Executable::build(&scenario, None).expect("Failed to build execution graph");
            executable.render()
        }
    };

    match args.output_file {
        Some(path) => {
            let mut file = File::create(path).expect("Failed to create output file");
            file.write(result.as_bytes())
                .expect("Failed to write to output file");
        }
        None => {
            println!("{}", result);
        }
    }
}
