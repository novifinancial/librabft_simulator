// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

//! Main executable to run a simulation of LibraBFT v2.

use bft_lib::{base_types::*, simulated_context::SimulatedContext, simulator, smr_context};
use clap::{App, Arg};
use librabft_v2::{data_sync::*, node::NodeState};
use log::{info, warn};

fn main() {
    let args = get_arguments();

    env_logger::init();
    let context_factory = |author, num_nodes| {
        let config = smr_context::Config {
            author,
            target_commit_interval: args.target_commit_interval,
            delta: args.delta,
            gamma: args.gamma,
            lambda: args.lambda,
        };
        SimulatedContext::new(config, num_nodes, args.commands_per_epoch)
    };
    let delay_distribution = simulator::RandomDelay::new(args.mean, args.variance);
    let mut sim = simulator::Simulator::<
        NodeState,
        SimulatedContext,
        DataSyncNotification,
        DataSyncRequest,
        DataSyncResponse,
    >::new(args.nodes, delay_distribution, context_factory);
    let contexts = sim.loop_until(
        simulator::GlobalTime(args.max_clock),
        args.output_data_files,
    );
    warn!("Commands executed per node: {:#?}", {
        let x: Vec<_> = contexts
            .iter()
            .map(|context| context.committed_history().len())
            .collect();
        x
    });
    info!("SMR contexts: {:#?}", contexts);
}

struct CliArguments {
    max_clock: i64,
    mean: f64,
    variance: f64,
    nodes: usize,
    commands_per_epoch: usize,
    target_commit_interval: Duration,
    delta: Duration,
    gamma: f64,
    lambda: f64,
    output_data_files: Option<String>,
}

// TODO: use structopt
fn get_arguments() -> CliArguments {
    let matches = App::new("Consensus simulator")
        .about("A monte-carlo simulation of the LibraBFT consensus protocol")
        .arg(
            Arg::with_name("max_clock")
                .long("max_clock")
                .help("Time at which to stop the simulation")
                .default_value("1000"),
        )
        .arg(
            Arg::with_name("mean")
                .long("mean")
                .help("The mean value of the normal distribution of the network delay")
                .default_value("10.0"),
        )
        .arg(
            Arg::with_name("variance")
                .long("variance")
                .help("The variance of the normal distribution of the network delay")
                .default_value("4.0"),
        )
        .arg(
            Arg::with_name("nodes")
                .long("nodes")
                .help("The number of nodes to simulate")
                .default_value("3"),
        )
        .arg(
            Arg::with_name("commands_per_epoch")
                .long("commands_per_epoch")
                .help("The maximum number of commands per epoch")
                .default_value("30000"),
        )
        .arg(
            Arg::with_name("target_commit_interval")
                .long("target_commit_interval")
                .help("Minimal interval between query-all actions when no commit happens")
                .default_value("100000"),
        )
        .arg(
            Arg::with_name("delta")
                .long("delta")
                .help("Maximal duration of the first round after a commit rule")
                .default_value("20"),
        )
        .arg(
            Arg::with_name("gamma")
                .long("gamma")
                .help("Exponent to increase round durations")
                .default_value("2.0"),
        )
        .arg(
            Arg::with_name("lambda")
                .long("lambda")
                .help("Coefficient to control the frequency of query-all actions")
                .default_value("0.5"),
        )
        .arg(Arg::with_name("create_csv").long("create_csv").help(
            "If given this argument, csv files will be generated with data on the simulation"
        ).takes_value(true))
        .get_matches();

    CliArguments {
        max_clock: matches
            .value_of("max_clock")
            .unwrap()
            .parse::<i64>()
            .unwrap(),
        mean: matches.value_of("mean").unwrap().parse::<f64>().unwrap(),
        variance: matches
            .value_of("variance")
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        nodes: matches.value_of("nodes").unwrap().parse::<usize>().unwrap(),
        commands_per_epoch: matches
            .value_of("commands_per_epoch")
            .unwrap()
            .parse::<usize>()
            .unwrap(),
        target_commit_interval: Duration(
            matches
                .value_of("target_commit_interval")
                .unwrap()
                .parse::<i64>()
                .unwrap(),
        ),
        delta: Duration(matches.value_of("delta").unwrap().parse::<i64>().unwrap()),
        gamma: matches.value_of("gamma").unwrap().parse::<f64>().unwrap(),
        lambda: matches.value_of("lambda").unwrap().parse::<f64>().unwrap(),
        output_data_files: matches.value_of("create_csv").map(|x| x.to_string()),
    }
}
