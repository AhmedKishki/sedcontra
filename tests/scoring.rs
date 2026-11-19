use sedcontra::eval::{Case, Landing, Outcome, Planted, score};
use sedcontra::model::{Critique, CritiqueStatus, Thread};

const PROPOSAL: &str = "A is true. B is true. So C is true.";

fn case(kind: &str, places: &[&str]) -> Case {
    let planted = Planted { kind: kind.into(), places: places.iter().map(|p| p.to_string()).collect(), note: None };
    Case::new("test".into(), PROPOSAL.into(), None, vec![planted]).unwrap()
}

fn thread(id: &str, kind: &str, places: &[&str], status: CritiqueStatus) -> Thread {
    Thread {
        critique: Critique {
            id: id.into(),
            kind: kind.into(),
            places: places.iter().map(|p| p.to_string()).collect(),
            point: "the step fails".into(),
            support: "the premises hold and the conclusion fails".into(),
        },
        raised_in: 1,
        replies: Vec::new(),
        status,
    }
}

#[test]
fn critique_at_the_defect_lands_with_its_status() {
    let case = case("gap", &["So C is true"]);
    let score = score(&case, &[thread("c1", "gap", &["So C is true"], CritiqueStatus::Accepted)]);
    assert_eq!(score.defects.len(), 1);
    assert_eq!(score.defects[0].outcome, Outcome::Accepted);
    assert!(score.defects[0].same_kind);
    assert!(score.unplanted.is_empty());
}

#[test]
fn overlapping_places_land() {
    let case = case("gap", &["So C is true"]);
    let inside = score(&case, &[thread("c1", "gap", &["C is true"], CritiqueStatus::Accepted)]);
    assert_ne!(inside.defects[0].outcome, Outcome::Missed);
    let across = score(&case, &[thread("c1", "gap", &["B is true. So C"], CritiqueStatus::Accepted)]);
    assert_ne!(across.defects[0].outcome, Outcome::Missed);
}

#[test]
fn critique_elsewhere_is_unplanted() {
    let case = case("gap", &["So C is true"]);
    let score = score(&case, &[thread("c1", "gap", &["A is true"], CritiqueStatus::Open)]);
    assert_eq!(score.defects[0].outcome, Outcome::Missed);
    assert_eq!(score.unplanted, vec![Landing { critique: "c1".into(), kind: "gap".into(), status: CritiqueStatus::Open }]);
}

#[test]
fn places_that_only_touch_do_not_overlap() {
    // The full stop begins where the defect ends.
    let defect_ends = case("gap", &["B is true"]);
    let stop_after = score(&defect_ends, &[thread("c1", "gap", &["."], CritiqueStatus::Accepted)]);
    assert_eq!(stop_after.defects[0].outcome, Outcome::Missed);
    assert_eq!(stop_after.unplanted.len(), 1);
    assert_eq!(stop_after.unplanted[0].critique, "c1");
    // With the roles swapped, the critique ends where the defect's occurrence of the full stop begins.
    let defect_stop = case("gap", &["."]);
    let quote_before = score(&defect_stop, &[thread("c1", "gap", &["B is true"], CritiqueStatus::Accepted)]);
    assert_eq!(quote_before.defects[0].outcome, Outcome::Missed);
}

#[test]
fn the_strongest_status_stands() {
    let case = case("gap", &["So C is true"]);
    let at = |id: &str, status| thread(id, "gap", &["So C is true"], status);
    let dropped_and_open = score(&case, &[at("c1", CritiqueStatus::Dropped), at("c2", CritiqueStatus::Open)]);
    assert_eq!(dropped_and_open.defects[0].outcome, Outcome::Open);
    let all_three = score(&case, &[at("c1", CritiqueStatus::Dropped), at("c2", CritiqueStatus::Open), at("c3", CritiqueStatus::Accepted)]);
    assert_eq!(all_three.defects[0].outcome, Outcome::Accepted);
    let dropped = score(&case, &[at("c1", CritiqueStatus::Dropped)]);
    assert_eq!(dropped.defects[0].outcome, Outcome::Dropped);
}

#[test]
fn a_landing_of_another_kind_is_not_the_same_kind() {
    let case = case("gap", &["So C is true"]);
    let score = score(&case, &[thread("c1", "conflict", &["So C is true"], CritiqueStatus::Accepted)]);
    assert_ne!(score.defects[0].outcome, Outcome::Missed);
    assert!(!score.defects[0].same_kind);
}

#[test]
fn a_place_that_occurs_twice_counts_at_every_occurrence() {
    // Only the last occurrence of the place reaches the defect.
    let case = case("gap", &["So C is true"]);
    let score = score(&case, &[thread("c1", "gap", &["is true"], CritiqueStatus::Accepted)]);
    assert_ne!(score.defects[0].outcome, Outcome::Missed);
}

#[test]
fn a_defect_place_missing_from_the_proposal_is_refused() {
    let planted = Planted { kind: "gap".into(), places: vec!["snow fell".into()], note: None };
    let failures = Case::new("test".into(), PROPOSAL.into(), None, vec![planted]).unwrap_err();
    assert!(failures.join("; ").contains("snow fell"), "{failures:?}");
}
