// Copyright (c) Facebook, Inc. and its affiliates.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    base_types::{Duration, NodeTime, Round},
    data_writer::DataWriter,
    interfaces::{ConsensusNode, DataSyncNode, NodeUpdateActions},
    simulated_context::Author,
    smr_context::SmrContext,
};
use futures::executor::block_on;
use log::{debug, trace};
use rand::{prelude::SliceRandom, SeedableRng};
use rand_distr::{Distribution, LogNormal};
use rand_xoshiro::Xoshiro256StarStar;
use std::{collections::BinaryHeap, fmt::Debug};

#[cfg(test)]
#[path = "unit_tests/simulator_tests.rs"]
mod simulator_tests;

/// Simulate the execution of a consensus protocol (including
/// configuration changes) over a randomized network.
///
/// TODO: simulate changing network conditions, addition/removal/disconnection of nodes, etc.
pub struct Simulator<Node, Context, Notification, Request, Response> {
    clock: GlobalTime,
    network_delay: RandomDelay,
    pending_events: BinaryHeap<ScheduledEvent<Event<Notification, Request, Response>>>,
    nodes: Vec<SimulatedNode<Node, Context>>,
    event_count: usize,
    rng: Xoshiro256StarStar,
}

/// Simulated global clock
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Debug)]
pub struct GlobalTime(pub i64);

/// A distribution that produces random delays.
#[derive(Copy, Clone, Debug)]
pub struct RandomDelay {
    distribution: LogNormal<f64>,
}

/// An event inserted in the binary heap.
/// Every event must have a unique `creation_stamp`.
struct ScheduledEvent<Event> {
    scheduled_time: GlobalTime,
    creation_stamp: usize,
    event: Event,
}

#[derive(Debug)]
pub struct SimulatedNode<Node, Context> {
    startup_time: GlobalTime,
    ignore_scheduled_updates_until: GlobalTime,
    node: Node,
    context: Context,
}

/// An event to be scheduled and processed by the simulator.
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Event<Notification, Request, Response> {
    DataSyncNotifyEvent {
        receiver: Author,
        sender: Author,
        notification: Notification,
    },
    DataSyncRequestEvent {
        receiver: Author,
        sender: Author,
        request: Request,
    },
    DataSyncResponseEvent {
        receiver: Author,
        sender: Author,
        response: Response,
    },
    UpdateTimerEvent {
        author: Author,
    },
}

// TODO: the notion of round is specific to some BFT protocols => rename and/or generalize?
/// Trait to help visualizing rounds in a simulator.
pub trait ActiveRound {
    fn active_round(&self) -> Round;
}

impl std::ops::Add<Duration> for GlobalTime {
    type Output = GlobalTime;

    fn add(self, rhs: Duration) -> Self::Output {
        GlobalTime(self.0 + rhs.0)
    }
}

impl RandomDelay {
    pub fn new(mean: f64, variance: f64) -> RandomDelay {
        // https://en.wikipedia.org/wiki/Log-normal_distribution
        let mu = f64::ln(mean / f64::sqrt(1.0 + variance / (mean * mean)));
        let sigma = f64::sqrt(f64::ln(1.0 + variance / (mean * mean)));
        RandomDelay {
            distribution: LogNormal::new(mu, sigma).unwrap(),
        }
    }
}

impl GlobalTime {
    fn add_delay<R: rand_core::RngCore + ?Sized>(
        self,
        rng: &mut R,
        delay: RandomDelay,
    ) -> GlobalTime {
        let v = delay.distribution.sample(rng);
        trace!("Picked random delay: {}", v);
        GlobalTime(self.0 + (v as i64))
    }

    fn to_node_time(self, startup_time: GlobalTime) -> NodeTime {
        NodeTime(self.0 - startup_time.0)
    }

    fn from_node_time(node_time: NodeTime, startup_time: GlobalTime) -> GlobalTime {
        GlobalTime(node_time.0 + startup_time.0)
    }
}

impl<Notification, Request, Response> Event<Notification, Request, Response> {
    fn kind(&self) -> usize {
        use Event::*;
        match self {
            DataSyncNotifyEvent { .. } => 0,
            DataSyncRequestEvent { .. } => 1,
            DataSyncResponseEvent { .. } => 2,
            UpdateTimerEvent { .. } => 3,
        }
    }
}

impl<Notification, Request, Response> PartialOrd
    for ScheduledEvent<Event<Notification, Request, Response>>
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl<Notification, Request, Response> Ord
    for ScheduledEvent<Event<Notification, Request, Response>>
{
    // std::collections::BinaryHeap is a max heap.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            other.scheduled_time,
            self.event.kind(),
            other.creation_stamp,
        )
            .cmp(&(self.scheduled_time, other.event.kind(), self.creation_stamp))
    }
}

impl<Event> PartialEq for ScheduledEvent<Event> {
    fn eq(&self, other: &Self) -> bool {
        self.creation_stamp == other.creation_stamp
    }
}

impl<Event> Eq for ScheduledEvent<Event> {}

impl<Node, Context> SimulatedNode<Node, Context>
where
    Node: ConsensusNode<Context>,
    Context: SmrContext,
{
    fn update(&mut self, global_clock: GlobalTime) -> NodeUpdateActions<Context> {
        let local_clock = global_clock.to_node_time(self.startup_time);
        self.node.update_node(&mut self.context, local_clock)
    }
}

impl<Node, Context> ActiveRound for SimulatedNode<Node, Context>
where
    Node: ActiveRound,
{
    fn active_round(&self) -> Round {
        self.node.active_round()
    }
}

impl<Node, Context, Notification, Request, Response>
    Simulator<Node, Context, Notification, Request, Response>
where
    Node: ConsensusNode<Context>,
    Context: SmrContext,
    Notification: Debug,
    Request: Debug,
    Response: Debug,
{
    pub fn new<F>(
        rng_seed: u64,
        num_nodes: usize,
        network_delay: RandomDelay,
        context_factory: F,
    ) -> Simulator<Node, Context, Notification, Request, Response>
    where
        F: Fn(Author, usize) -> Context,
    {
        let clock = GlobalTime(0);
        let mut pending_events = BinaryHeap::new();
        let mut event_count = 0;
        let mut rng = rand_xoshiro::Xoshiro256StarStar::seed_from_u64(rng_seed);
        let nodes = (0..num_nodes)
            .map(|index| {
                let author = Author(index);
                let mut context = context_factory(author, num_nodes);
                let startup_time = clock.add_delay(&mut rng, network_delay) + Duration(1);
                let node_time = NodeTime(0);
                let scheduled_time = GlobalTime::from_node_time(node_time, startup_time);
                let event = Event::UpdateTimerEvent { author };
                let node = block_on(Node::load_node(&mut context, node_time))
                    .expect("loading nodes in simulator should not fail");
                trace!(
                    "Scheduling initial event {:?} for time {:?}",
                    event,
                    scheduled_time
                );
                pending_events.push(ScheduledEvent {
                    scheduled_time,
                    creation_stamp: event_count,
                    event,
                });
                event_count += 1;
                SimulatedNode {
                    startup_time,
                    ignore_scheduled_updates_until: startup_time + Duration(-1),
                    node,
                    context,
                }
            })
            .collect();
        Simulator {
            clock,
            network_delay,
            pending_events,
            nodes,
            event_count,
            rng,
        }
    }

    fn schedule_event(
        &mut self,
        scheduled_time: GlobalTime,
        event: Event<Notification, Request, Response>,
    ) {
        trace!("Scheduling event {:?} for {:?}", event, scheduled_time);
        self.pending_events.push(ScheduledEvent {
            scheduled_time,
            creation_stamp: self.event_count,
            event,
        });
        self.event_count += 1;
    }

    fn schedule_network_event(&mut self, event: Event<Notification, Request, Response>) {
        let scheduled_time = self.clock.add_delay(&mut self.rng, self.network_delay);
        self.schedule_event(scheduled_time, event);
    }
}

impl<Node, Context, Notification, Request, Response>
    Simulator<Node, Context, Notification, Request, Response>
{
    pub fn simulated_node(&self, author: Author) -> &SimulatedNode<Node, Context> {
        self.nodes.get(author.0).unwrap()
    }

    fn simulated_node_mut(&mut self, author: Author) -> &mut SimulatedNode<Node, Context> {
        self.nodes.get_mut(author.0).unwrap()
    }
}

impl<Node, Context, Notification, Request, Response>
    Simulator<Node, Context, Notification, Request, Response>
where
    Context: SmrContext<Author = Author>,
    Node: ConsensusNode<Context>
        + DataSyncNode<Context, Notification = Notification, Request = Request, Response = Response>
        + ActiveRound
        + Debug,
    Notification: Debug + Clone,
    Request: Debug + Clone,
    Response: Debug,
{
    fn process_node_actions(
        &mut self,
        clock: GlobalTime,
        author: Author,
        actions: NodeUpdateActions<Context>,
    ) {
        debug!(
            "@{:?} Processing node actions for {:?}: {:?}",
            clock, author, actions
        );
        // First, we must save the state of the node.
        let mut node = self.simulated_node_mut(author);
        block_on(node.node.save_node(&mut node.context))
            .expect("saving nodes should not fail in simulator");
        // Then, schedule the next call to `update_node`.
        let new_scheduled_time = {
            let new_scheduled_time = std::cmp::max(
                GlobalTime::from_node_time(actions.next_scheduled_update, node.startup_time),
                // Make sure we schedule the update strictly in the future so it does not get
                // ignored by `ignore_scheduled_updates_until` below.
                clock + Duration(1),
            );
            // We don't remove the previously scheduled updates but this will cancel them.
            node.ignore_scheduled_updates_until = new_scheduled_time + Duration(-1);
            new_scheduled_time
            // scoping the mutable 'node' for the borrow checker
        };
        let event = Event::UpdateTimerEvent { author };
        self.schedule_event(new_scheduled_time, event);
        // Schedule sending notifications.
        let mut receivers = Vec::new();
        if actions.should_broadcast {
            // TODO: broadcasting to all (past and future) nodes in the network is not entirely
            // realistic. The pseudo-code should probably use `actions.should_send` instead to
            // broadcast only to the nodes that a sender consider part of the epoch.
            for index in 0..self.nodes.len() {
                if index != author.0 {
                    receivers.push(Author(index));
                }
            }
        } else {
            for receiver in actions.should_send {
                if receiver != author {
                    receivers.push(receiver);
                }
            }
        }
        receivers.shuffle(&mut self.rng);
        let notification = {
            let node = self.simulated_node(author);
            node.node.create_notification(&node.context)
        };
        for receiver in receivers {
            self.schedule_network_event(Event::DataSyncNotifyEvent {
                sender: author,
                receiver,
                notification: notification.clone(),
            });
        }
        // Schedule sending requests.
        let mut senders = Vec::new();
        if actions.should_query_all {
            // TODO: similarly `should_query_all` is probably too coarse.
            for index in 0..self.nodes.len() {
                if index != author.0 {
                    senders.push(Author(index));
                }
            }
        }
        let request = {
            let node = self.simulated_node(author);
            node.node.create_request(&node.context)
        };
        let mut senders = senders.into_iter().collect::<Vec<_>>();
        senders.shuffle(&mut self.rng);
        for sender in senders {
            self.schedule_network_event(Event::DataSyncRequestEvent {
                receiver: author,
                sender,
                request: request.clone(),
            });
        }
    }

    pub fn loop_until(&mut self, max_clock: GlobalTime, csv_path: Option<String>) -> Vec<&Context> {
        let mut data_writer = { csv_path.map(|path| DataWriter::new(self.nodes.len(), path)) };

        while let Some(ScheduledEvent {
            scheduled_time: clock,
            event,
            ..
        }) = self.pending_events.pop()
        {
            if clock > max_clock {
                break;
            }

            if let Some(data_writer_val) = data_writer.as_mut() {
                data_writer_val.update_round_number(&self, &clock);
                data_writer_val.add_message_counter(&event);
            }

            // Events scheduled in the past are fine but they do not move the clock.
            let clock = std::cmp::max(clock, self.clock);
            self.clock = clock;
            debug!("@{:?} Processing event {:?}", clock, event);
            match event {
                Event::UpdateTimerEvent { author } => {
                    let actions = {
                        let node = self.simulated_node_mut(author);
                        if clock <= node.ignore_scheduled_updates_until {
                            // This scheduled update was invalidated in the meantime.
                            debug!("@{:?} Timer was cancelled: {:?}", clock, event);
                            continue;
                        }
                        node.update(clock)
                    };
                    trace!("Node state: {:?}", self.simulated_node(author));
                    self.process_node_actions(clock, author, actions);
                }
                Event::DataSyncNotifyEvent {
                    receiver,
                    sender,
                    notification,
                } => {
                    let node = self.simulated_node_mut(receiver);
                    let result = block_on(
                        node.node
                            .handle_notification(&mut node.context, notification),
                    );
                    let actions = node.update(clock);
                    if let Some(request) = result {
                        self.schedule_network_event(Event::DataSyncRequestEvent {
                            sender,
                            receiver,
                            request,
                        });
                    }
                    trace!(
                        "Node state: {:?}, node index: {:?}",
                        self.simulated_node(receiver),
                        receiver
                    );
                    self.process_node_actions(clock, receiver, actions);
                }
                Event::DataSyncRequestEvent {
                    receiver,
                    sender,
                    request,
                } => {
                    let node = self.simulated_node_mut(receiver);
                    let response = block_on(node.node.handle_request(&mut node.context, request));
                    self.schedule_network_event(Event::DataSyncResponseEvent {
                        sender,
                        receiver,
                        response,
                    });
                }
                Event::DataSyncResponseEvent {
                    receiver, response, ..
                } => {
                    let node = self.simulated_node_mut(receiver);
                    let local_clock = clock.to_node_time(node.startup_time);
                    block_on(
                        node.node
                            .handle_response(&mut node.context, response, local_clock),
                    );
                    let actions = node.update(clock);
                    trace!("Node state: {:?}", node);
                    self.process_node_actions(clock, receiver, actions);
                }
            }
        }

        if let Some(data_writer_val) = data_writer {
            data_writer_val.write_to_file();
        }

        self.nodes.iter().map(|node| &node.context).collect()
    }
}
