use predicates::prelude::*;
use serde_json::json;

use crate::helpers::{self, TestEnv};

#[test]
fn monitor_polls_and_updates_cache() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");

    let mut server = mockito::Server::new();

    let _m1 = server
        .mock("GET", "/api/oauth/usage")
        .match_header("Authorization", "Bearer sk-ant-oat01-fake-personal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&helpers::fake_api_response(50.0, 30.0)).unwrap())
        .create();

    let _m2 = server
        .mock("GET", "/api/oauth/usage")
        .match_header("Authorization", "Bearer sk-ant-oat01-fake-work")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&helpers::fake_api_response(20.0, 60.0)).unwrap())
        .create();

    env.cmd_with_mock(&server)
        .arg("monitor")
        .assert()
        .success();

    let cache = env.read_usage_cache();
    // Personal: 5h=50%, 7d=30%
    assert_eq!(cache["personal"]["five_hour"]["utilization"], 50.0);
    assert_eq!(cache["personal"]["seven_day"]["utilization"], 30.0);
    assert_eq!(cache["personal"]["token_expired"], false);
    // Work: 5h=20%, 7d=60%
    assert_eq!(cache["work"]["five_hour"]["utilization"], 20.0);
    assert_eq!(cache["work"]["seven_day"]["utilization"], 60.0);
}

#[test]
fn monitor_marks_expired_on_401() {
    let env = TestEnv::new();
    env.add_account("expired-acct");
    env.set_active("expired-acct");

    let mut server = mockito::Server::new();

    let _m = server
        .mock("GET", "/api/oauth/usage")
        .match_header("Authorization", "Bearer sk-ant-oat01-fake-expired-acct")
        .with_status(401)
        .with_body("Unauthorized")
        .create();

    env.cmd_with_mock(&server)
        .arg("monitor")
        .assert()
        .success();

    let cache = env.read_usage_cache();
    assert_eq!(cache["expired-acct"]["token_expired"], true);
}

#[test]
fn monitor_no_accounts_exits_ok() {
    let env = TestEnv::new();

    env.cmd()
        .arg("monitor")
        .assert()
        .success()
        .stderr(predicate::str::contains("no accounts"));
}

#[test]
fn monitor_handles_partial_response() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    let mut server = mockito::Server::new();
    // Only five_hour, no seven_day
    let _m = server
        .mock("GET", "/api/oauth/usage")
        .match_header("Authorization", "Bearer sk-ant-oat01-fake-personal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"five_hour":{"utilization":42.0,"resets_at":"2026-03-18T10:00:00Z"}}"#)
        .create();

    env.cmd_with_mock(&server)
        .arg("monitor")
        .assert()
        .success();

    let cache = env.read_usage_cache();
    assert_eq!(cache["personal"]["five_hour"]["utilization"], 42.0);
    assert!(cache["personal"]["seven_day"].is_null());
}
