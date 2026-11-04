//! The Host protocol fixture shared with packages/host: the JSON shapes and the reducer
//! must agree on both sides (D6).

use serde_json::Value;
use terminescu_core::state::{Command, PatchBatch, Snapshot, Workspace, apply};

const FIXTURE: &str = include_str!("../../../packages/host/fixtures/host-protocol.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

#[test]
fn reducer_turns_start_plus_batches_into_end() {
    let f = fixture();
    let mut ws = serde_json::from_value::<Snapshot>(f["start"].clone()).unwrap().workspace;
    for b in f["batches"].as_array().unwrap() {
        let batch: PatchBatch = serde_json::from_value(b.clone()).unwrap();
        for p in &batch.patches {
            apply(&mut ws, p).unwrap();
        }
    }
    let end: Workspace = serde_json::from_value(f["end"].clone()).unwrap();
    assert_eq!(ws, end);
}

#[test]
fn every_fixture_message_roundtrips_to_the_same_json() {
    let f = fixture();
    for c in f["commands"].as_array().unwrap() {
        let cmd: Command = serde_json::from_value(c.clone()).unwrap();
        assert_eq!(&serde_json::to_value(&cmd).unwrap(), c);
    }
    for b in f["batches"].as_array().unwrap() {
        let batch: PatchBatch = serde_json::from_value(b.clone()).unwrap();
        assert_eq!(&serde_json::to_value(&batch).unwrap(), b);
    }
    let snap: Snapshot = serde_json::from_value(f["start"].clone()).unwrap();
    assert_eq!(serde_json::to_value(&snap).unwrap(), f["start"]);
}
