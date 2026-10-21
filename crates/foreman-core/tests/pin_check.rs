//! CR-003: the setup preview's read-only pin check (`lock::p1_annotation_errors`) finds exactly
//! what `lock::pin` refuses on.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

mod common;

use common::{blueprint, replace_once};
use foreman_core::lock::p1_annotation_errors;
use foreman_core::lock::{self as lk, BlueprintState, LockError};
use foreman_core::store::{Root, RootKind};

fn pin(src: &str) -> Result<(), LockError> {
    let project = tempfile::tempdir().unwrap();
    let app = tempfile::tempdir().unwrap();
    let oid = "1111111111111111111111111111111111111111".to_owned();
    let state = BlueprintState {
        bytes: src.as_bytes().to_vec(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        working_oid: oid.clone(),
        head_oid: Some(oid),
    };
    lk::pin(
        &Root::open(project.path(), RootKind::Project).unwrap(),
        &Root::open_app_data(app.path()).unwrap(),
        "claude-build-blueprint.md",
        &state,
        false,
        "2026-10-20T10:00:00Z",
    )
    .map(|_| ())
}

fn problems(r: Result<(), LockError>) -> Option<(String, Vec<lk::AnnotationProblem>)> {
    match r {
        Ok(()) => None,
        Err(e) => {
            let text = e.to_string();
            match e {
                LockError::AnnotationErrors { problems } => Some((text, problems)),
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}

#[test]
fn preview_check_matches_pin() {
    let src = blueprint();
    let one = replace_once(&src, "<!-- fm:step id=2.5 ", "<!-- fm:step id=2.5 bogus=1 ");
    let several = replace_once(
        &replace_once(
            &src,
            "id=2.3 kind=followup tag=core surface=cli session=continue chain=2.2",
            "id=2.3 kind=followup tag=core surface=cli session=continue chain=2.9",
        ),
        "id=1.2 tag=core session=continue chain=1.1",
        "id=1.2 tag=core session=continue chain=1.9",
    );
    for s in [&src, &one, &several] {
        assert_eq!(problems(p1_annotation_errors(s)), problems(pin(s)));
    }
    assert!(problems(p1_annotation_errors(&one)).is_some());
    assert!(p1_annotation_errors(&src).is_ok());
}
