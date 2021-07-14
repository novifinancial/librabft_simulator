// Copyright(C) Facebook, Inc. and its affiliates.
use crate::batch_maker::{Batch, Transaction};

// Fixture
pub fn transaction() -> Transaction {
    vec![0; 100]
}

// Fixture
pub fn batch() -> Batch {
    vec![transaction(), transaction()]
}
