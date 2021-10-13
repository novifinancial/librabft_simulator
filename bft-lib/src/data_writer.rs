// Copyright (c) Facebook, Inc. and its affiliates.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    simulated_context::Author,
    simulator::{ActiveRound, Event, GlobalTime, Simulator},
};
use std::{fs, path::Path};

pub struct DataWriter {
    data_files_path: String,
    nodes_len: usize,
    // Variables for monitoring round switches
    max_round_per_node: Vec<usize>,
    nodes_round_switch: Vec<Vec<(usize, GlobalTime)>>,
    message_counter: usize, // Counts the number of messages
}

impl DataWriter {
    pub fn new(nodes_num: usize, path: String) -> DataWriter {
        let data_writer = DataWriter {
            nodes_len: nodes_num,
            max_round_per_node: vec![0; nodes_num],
            nodes_round_switch: vec![Vec::new(); nodes_num],
            data_files_path: path,
            message_counter: 0,
        };
        if !Path::new(&data_writer.data_files_path).exists() {
            fs::create_dir(&data_writer.data_files_path).expect("could not create result dir");
        }
        data_writer
    }

    pub fn update_round_number<State, Context, Notification, Request, Response>(
        &mut self,
        simulator: &Simulator<State, Context, Notification, Request, Response>,
        clock: &GlobalTime,
    ) where
        State: ActiveRound,
    {
        for node_num in 0..self.nodes_len {
            let node = simulator.simulated_node(Author(node_num));
            let node_round = node.active_round().0;
            if node_round > *self.max_round_per_node.get(node_num).unwrap() {
                self.max_round_per_node[node_num] = node_round;
                self.nodes_round_switch[node_num].push((node_round, *clock))
            }
        }
    }

    pub fn add_message_counter<Notification, Request, Response>(
        &mut self,
        event: &Event<Notification, Request, Response>,
    ) {
        match event {
            Event::UpdateTimerEvent { author: _ } => {}
            _ => self.message_counter += 1,
        }
    }

    pub fn write_to_file(&self) {
        let mut wtr =
            csv::Writer::from_path(format!("{}/{}", self.data_files_path, "round_switches.txt"))
                .unwrap();

        // CSV of the round switch
        let headers: Vec<_> = (0..self.nodes_len).collect();
        let headers: Vec<String> = headers
            .iter()
            .map(|x| format!("node {}", x.to_string()))
            .collect();
        wtr.serialize(&headers).expect("writing did not succeed");

        let max_round = *self.max_round_per_node.iter().max().unwrap() as i32;
        for round_num in 0..max_round {
            let mut time_row: Vec<Option<i64>> = Vec::new();
            for node_num in 0..self.nodes_len {
                let time = self.nodes_round_switch[node_num]
                    .iter()
                    .find(|&x| x.0 == round_num as usize);
                match time {
                    Some(time) => time_row.push(Some((time.1).0)),
                    None => time_row.push(None),
                };
            }
            wtr.serialize(time_row).expect("Writing did not succeed");
        }

        let mut wtr = csv::Writer::from_path(format!(
            "{}/{}",
            self.data_files_path, "number_of_messages.txt"
        ))
        .unwrap();
        wtr.serialize(Some(self.message_counter))
            .expect("Writing did not succeed");
    }
}
