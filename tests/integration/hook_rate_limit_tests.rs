use crate::helpers::TestEnv;

#[test]
fn rate_limit_creates_flag_on_type() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(r#"{"error":{"type":"rate_limit_error","message":"Rate limit exceeded"}}"#)
        .assert()
        .success();

    assert!(env.rate_limited_exists());
}

#[test]
fn rate_limit_creates_flag_on_message() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(r#"{"error":{"type":"unknown","message":"usage limit reached"}}"#)
        .assert()
        .success();

    assert!(env.rate_limited_exists());
}

#[test]
fn rate_limit_creates_flag_on_case_variation() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(r#"{"error":{"type":"unknown","message":"Rate limit hit"}}"#)
        .assert()
        .success();

    assert!(env.rate_limited_exists());
}

#[test]
fn rate_limit_ignores_other_errors() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(r#"{"error":{"type":"api_error","message":"Server error"}}"#)
        .assert()
        .success();

    assert!(!env.rate_limited_exists());
}

#[test]
fn rate_limit_ignores_null_error() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(r#"{"error":null}"#)
        .assert()
        .success();

    assert!(!env.rate_limited_exists());
}

#[test]
fn rate_limit_malformed_noop() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin("not json")
        .assert()
        .success();

    assert!(!env.rate_limited_exists());
}

#[test]
fn rate_limit_flag_contains_timestamp() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(r#"{"error":{"type":"rate_limit_error","message":"limit"}}"#)
        .assert()
        .success();

    let content = std::fs::read_to_string(env.root.path().join("data/rate-limited")).unwrap();
    // Should be a valid RFC 3339 timestamp
    assert!(content.contains("T"));
    assert!(content.contains("Z") || content.contains("+"));
}
