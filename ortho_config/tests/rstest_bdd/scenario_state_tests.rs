//! Regression tests for scenario-state recovery behaviour.

use super::LocalizerContext;
use rstest::rstest;
use std::sync::{Arc, PoisonError};

#[rstest]
fn take_issues_recovers_from_poisoned_mutex() {
    let context = LocalizerContext {
        issues: LocalizerContext::init_issues(),
        ..LocalizerContext::default()
    };
    let issues = context
        .issues
        .with_ref(Arc::clone)
        .expect("issues state should be initialized");

    let poisoning_thread = std::thread::spawn(move || {
        let mut guard = issues.lock().unwrap_or_else(PoisonError::into_inner);
        guard.push("captured issue".to_owned());
        panic!("poison issues mutex deliberately");
    });
    assert!(poisoning_thread.join().is_err());

    assert_eq!(context.take_issues(), vec!["captured issue"]);
    assert!(context.take_issues().is_empty());
}
