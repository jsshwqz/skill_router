use std::fs;

use aion_router::error_kb::{ErrorKnowledgeBase, ErrorLifecycle, ErrorObservation, GateDecision, VerifiedFix};
use aion_router::learner::SkillLearner;
use serde_json::json;

fn observation(error: &str, context: serde_json::Value) -> ErrorObservation {
    ErrorObservation {
        capability: "mcp_call".to_string(),
        error_class: "runtime_error".to_string(),
        error: error.to_string(),
        context,
        version: "0.8.0".to_string(),
        observed_at: 10,
    }
}

#[test]
fn fingerprint_is_stable_for_normalized_error_and_context_order() {
    let left = observation(
        "Connection   refused at 127.0.0.1:3000",
        json!({"server":"local", "args":{"b":2,"a":1}}),
    );
    let right = observation(
        " connection refused at 127.0.0.1:3000 ",
        json!({"args":{"a":1,"b":2}, "server":"local"}),
    );
    assert_eq!(left.fingerprint(), right.fingerprint());
}

#[test]
fn verified_recurrence_becomes_regressed_and_increases_priority() {
    let mut knowledge = ErrorKnowledgeBase::default();
    let observed = observation("connection refused", json!({"server":"local"}));
    let fingerprint = knowledge.observe(observed.clone());
    knowledge.mark_reproduced(&fingerprint).unwrap();
    knowledge
        .mark_fixed(
            &fingerprint,
            VerifiedFix {
                commit: "abc123".to_string(),
                pull_request: Some("#8".to_string()),
                regression_test: "mcp_reconnects_after_restart".to_string(),
                mitigation: Some("restart managed transport".to_string()),
            },
        )
        .unwrap();
    knowledge.mark_verified(&fingerprint, "0.8.0", 20).unwrap();
    let previous_priority = knowledge.get(&fingerprint).unwrap().priority;

    knowledge.observe(ErrorObservation {
        observed_at: 30,
        ..observed
    });

    let record = knowledge.get(&fingerprint).unwrap();
    assert_eq!(record.lifecycle, ErrorLifecycle::Regressed);
    assert!(record.priority > previous_priority);
    assert_eq!(record.recurrence_count, 1);
}

#[test]
fn success_resolves_only_the_matching_fingerprint() {
    let mut knowledge = ErrorKnowledgeBase::default();
    let matching = observation("connection refused", json!({"server":"local"}));
    let other = observation("connection refused", json!({"server":"remote"}));
    let matching_id = knowledge.observe(matching.clone());
    let other_id = knowledge.observe(other);

    knowledge.resolve_success(&matching, "0.8.1", 40);

    assert_eq!(knowledge.get(&matching_id).unwrap().lifecycle, ErrorLifecycle::Fixed);
    assert_eq!(knowledge.get(&other_id).unwrap().lifecycle, ErrorLifecycle::Observed);
}

#[test]
fn prevention_gate_blocks_unresolved_and_applies_verified_mitigation() {
    let mut knowledge = ErrorKnowledgeBase::default();
    let unresolved = observation("permission denied", json!({"server":"unsafe"}));
    knowledge.observe(unresolved.clone());
    assert!(matches!(
        knowledge.pre_execution("mcp_call", &unresolved.context),
        GateDecision::Block { .. }
    ));

    let verified = observation("connection refused", json!({"server":"local"}));
    let fingerprint = knowledge.observe(verified.clone());
    knowledge.mark_reproduced(&fingerprint).unwrap();
    knowledge
        .mark_fixed(
            &fingerprint,
            VerifiedFix {
                commit: "abc123".to_string(),
                pull_request: None,
                regression_test: "reconnect".to_string(),
                mitigation: Some("restart managed transport".to_string()),
            },
        )
        .unwrap();
    knowledge.mark_verified(&fingerprint, "0.8.0", 50).unwrap();
    assert!(matches!(
        knowledge.pre_execution("mcp_call", &verified.context),
        GateDecision::ApplyKnownMitigation { .. }
    ));
}

#[test]
fn records_persist_and_old_records_load_with_defaults() {
    let root = std::env::temp_dir().join(format!("aion-error-kb-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("errors.json");
    fs::write(
        &path,
        r#"[{"fingerprint":"legacy","capability":"echo","error_class":"runtime_error","normalized_error":"failed","context_signature":"{}"}]"#,
    )
    .unwrap();

    let knowledge = ErrorKnowledgeBase::load(&path).unwrap();
    let record = knowledge.get("legacy").unwrap();
    assert_eq!(record.lifecycle, ErrorLifecycle::Observed);
    assert_eq!(record.priority, 1);
    knowledge.save(&path).unwrap();
    assert!(path.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_success_fingerprint_cannot_resolve_another_error() {
    let mut knowledge = ErrorKnowledgeBase::default();
    let first = observation("connection refused", json!({"server":"local"}));
    let second = observation("permission denied", json!({"server":"local"}));
    let first_id = knowledge.observe(first);
    let second_id = knowledge.observe(second);

    knowledge.resolve_fingerprint_success(&first_id, "0.8.1", 60).unwrap();

    assert_eq!(knowledge.get(&first_id).unwrap().lifecycle, ErrorLifecycle::Fixed);
    assert_eq!(knowledge.get(&second_id).unwrap().lifecycle, ErrorLifecycle::Observed);
}

#[test]
fn learner_persists_failures_and_prevents_repeating_them() {
    let root = std::env::temp_dir().join(format!("aion-learner-kb-{}", uuid::Uuid::new_v4()));
    let learner = SkillLearner::load(&root, &root);
    let context = json!({"server":"unsafe"});
    let fingerprint = learner
        .observe_error("mcp_call", "runtime_error", "permission denied", &context, "0.8.0", 70)
        .unwrap();
    assert!(matches!(
        learner.pre_execution_gate("mcp_call", &context),
        GateDecision::Block { .. }
    ));

    drop(learner);
    let reloaded = SkillLearner::load(&root, &root);
    assert!(reloaded.error_record(&fingerprint).is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn learner_advances_error_through_verified_lifecycle() {
    let root = std::env::temp_dir().join(format!("aion-learner-lifecycle-{}", uuid::Uuid::new_v4()));
    let learner = SkillLearner::load(&root, &root);
    let context = json!({"server":"local"});
    let fingerprint = learner
        .observe_error("mcp_call", "runtime_error", "connection refused", &context, "0.8.0", 80)
        .unwrap();

    learner.mark_error_reproduced(&fingerprint).unwrap();
    learner
        .mark_error_fixed(
            &fingerprint,
            VerifiedFix {
                commit: "abc123".to_string(),
                pull_request: Some("#8".to_string()),
                regression_test: "mcp_reconnects_after_restart".to_string(),
                mitigation: Some("restart managed transport".to_string()),
            },
        )
        .unwrap();
    learner.mark_error_verified(&fingerprint, "0.8.1", 90).unwrap();

    let record = learner.error_record(&fingerprint).unwrap();
    assert_eq!(record.lifecycle, ErrorLifecycle::Verified);
    assert_eq!(record.replay.verified_versions, vec!["0.8.1"]);
    assert!(matches!(
        learner.pre_execution_gate("mcp_call", &context),
        GateDecision::ApplyKnownMitigation { .. }
    ));
    fs::remove_dir_all(root).unwrap();
}
