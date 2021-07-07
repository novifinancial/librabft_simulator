// Copyright (c) Calibra Research
// SPDX-License-Identifier: Apache-2.0

use super::*;
use bft_lib::simulated_context::SimulatedContext;
use std::collections::BTreeSet;

#[test]
fn test_serde_notification() {
    let data = DataSyncNotification::<SimulatedContext> {
        current_epoch: EpochId(0),
        highest_commit_certificate: None,
        highest_quorum_certificate: None,
        timeouts: Vec::new(),
        current_vote: None,
        proposed_block: None,
    };
    let message = serde_json::to_string(&data).unwrap();
    let data2: DataSyncNotification<SimulatedContext> = serde_json::from_str(&message).unwrap();
    assert_eq!(data2, data);
}

#[test]
fn test_serde_request() {
    let data = DataSyncRequest {
        current_epoch: EpochId(0),
        known_quorum_certificates: BTreeSet::default(),
    };
    let message = serde_json::to_string(&data).unwrap();
    let data2: DataSyncRequest = serde_json::from_str(&message).unwrap();
    assert_eq!(data2, data);
}

#[test]
fn test_serde_response() {
    let data = DataSyncResponse::<SimulatedContext> {
        current_epoch: EpochId(0),
        records: Vec::new(),
    };
    let message = serde_json::to_string(&data).unwrap();
    let data2: DataSyncResponse<SimulatedContext> = serde_json::from_str(&message).unwrap();
    assert_eq!(data2, data);
}
