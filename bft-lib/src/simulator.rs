// Copyright (c) Calibra Research
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
use rand_distr::{Distribution, LogNormal};
use std::collections::{BinaryHeap, HashSet};

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
    pending_events: PendingEvents<Notification, Request, Response>,
    nodes: Vec<SimulatedNode<Node, Context>>,
}

/// Simulated global clock
#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Debug)]
pub struct GlobalTime(pub i64);

#[derive(Copy, Clone)]
pub struct RandomDelay {
    distribution: LogNormal<f64>,
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct ScheduledEvent<Notification, Request, Response>(
    std::cmp::Reverse<GlobalTime>,
    Event<Notification, Request, Response>,
);

#[derive(Debug)]
pub struct SimulatedNode<Node, Context> {
    startup_time: GlobalTime,
    ignore_scheduled_updates_until: GlobalTime,
    node: Node,
    context: Context,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug)]
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
    fn add_delay(self, delay: RandomDelay) -> GlobalTime {
        let v = delay.distribution.sample(&mut rand::thread_rng());
        GlobalTime(self.0 + (v as i64))
    }

    fn to_node_time(self, startup_time: GlobalTime) -> NodeTime {
        NodeTime(self.0 - startup_time.0)
    }

    fn from_node_time(node_time: NodeTime, startup_time: GlobalTime) -> GlobalTime {
        GlobalTime(node_time.0 + startup_time.0)
    }
}

type PendingEvents<Notification, Request, Response> =
    BinaryHeap<ScheduledEvent<Notification, Request, Response>>;

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
    Notification: std::cmp::Ord + std::fmt::Debug,
    Request: std::cmp::Ord + std::fmt::Debug,
    Response: std::cmp::Ord + std::fmt::Debug,
{
    pub fn new<F>(
        num_nodes: usize,
        network_delay: RandomDelay,
        context_factory: F,
    ) -> Simulator<Node, Context, Notification, Request, Response>
    where
        F: Fn(Author, usize) -> Context,
    {
        let clock = GlobalTime(0);
        let mut pending_events = BinaryHeap::new();
        let nodes = (0..num_nodes)
            .map(|index| {
                let author = Author(index);
                let mut context = context_factory(author, num_nodes);
                let startup_time = clock.add_delay(network_delay) + Duration(1);
                let node_time = NodeTime(0);
                let deadline = GlobalTime::from_node_time(node_time, startup_time);
                let event = Event::UpdateTimerEvent { author };
                let node = block_on(Node::load_node(&mut context, node_time));
                trace!(
                    "Scheduling initial event {:?} for time {:?}",
                    event,
                    deadline
                );
                pending_events.push(ScheduledEvent(std::cmp::Reverse(deadline), event));
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
        }
    }

    fn schedule_event(
        &mut self,
        deadline: GlobalTime,
        event: Event<Notification, Request, Response>,
    ) {
        trace!("Scheduling event {:?} for {:?}", event, deadline);
        self.pending_events
            .push(ScheduledEvent(std::cmp::Reverse(deadline), event));
    }

    fn schedule_network_event(&mut self, event: Event<Notification, Request, Response>) {
        let deadline = self.clock.add_delay(self.network_delay);
        self.schedule_event(deadline, event);
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
        + std::fmt::Debug,
    Notification: std::cmp::Ord + std::fmt::Debug + std::clone::Clone,
    Request: std::cmp::Ord + std::fmt::Debug + std::clone::Clone,
    Response: std::cmp::Ord + std::fmt::Debug,
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
        let mut node = self.nodes.get_mut(author.0).unwrap();
        // First, we must save the state of the node.
        block_on(node.node.save_node(&mut node.context));
        // Then, schedule the next call to `update_node`.
        let new_deadline = {
            let new_deadline = std::cmp::max(
                GlobalTime::from_node_time(actions.next_scheduled_update, node.startup_time),
                // Make sure we schedule the update strictly in the future so it does not get
                // ignored by `ignore_scheduled_updates_until` below.
                clock + Duration(1),
            );
            // We don't remove the previously scheduled updates but this will cancel them.
            node.ignore_scheduled_updates_until = new_deadline + Duration(-1);
            new_deadline
            // scoping the mutable 'node' for the borrow checker
        };
        let event = Event::UpdateTimerEvent { author };
        self.schedule_event(new_deadline, event);
        // Schedule sending notifications.
        let mut receivers = HashSet::new();
        for node in actions.should_send {
            receivers.insert(node);
        }
        if actions.should_broadcast {
            // TODO: broadcasting to all (past and future) nodes in the network is not entirely
            // realistic. The pseudo-code should probably use `actions.should_send` instead to
            // broadcast only to the nodes that a sender consider part of the epoch.
            for index in 0..self.nodes.len() {
                if index != author.0 {
                    receivers.insert(Author(index));
                }
            }
        }
        let notification = self.simulated_node(author).node.create_notification();
        for receiver in receivers {
            self.schedule_network_event(Event::DataSyncNotifyEvent {
                sender: author,
                receiver,
                notification: notification.clone(),
            });
        }
        // Schedule sending requests.
        let mut senders = HashSet::new();
        if actions.should_query_all {
            // TODO: similarly `should_query_all` is probably too coarse.
            for index in 0..self.nodes.len() {
                if index != author.0 {
                    senders.insert(Author(index));
                }
            }
        }
        let request = self.simulated_node(author).node.create_request();
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

        while let Some(ScheduledEvent(std::cmp::Reverse(clock), event)) = self.pending_events.pop()
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
