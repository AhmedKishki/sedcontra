use sedcontra::check::{QuoteIndex, check_tree};
use sedcontra::model::{Claim, Inference, Origin, Tree};

fn claim(id: &str, places: &[&str]) -> Claim {
    Claim {
        id: id.into(),
        text: id.into(),
        places: places.iter().map(|p| p.to_string()).collect(),
        origin: None,
        backed_by_context: false,
    }
}

fn inf(id: &str, premises: &[&str], conclusion: &str) -> Inference {
    Inference {
        id: id.into(),
        premises: premises.iter().map(|p| p.to_string()).collect(),
        conclusion: conclusion.into(),
        places: vec![],
        origin: None,
    }
}

fn tree(claims: Vec<Claim>, inferences: Vec<Inference>, conclusions: &[&str]) -> Tree {
    Tree { claims, inferences, conclusions: conclusions.iter().map(|c| c.to_string()).collect() }
}

fn errors_of(proposal: &str, t: Tree) -> Vec<String> {
    check_tree(&QuoteIndex::new(proposal), t).expect_err("expected the check to fail")
}

const ABC: &str = "A holds. B holds. C holds. Q holds. X holds. Y holds. Z holds.";

#[test]
fn quote_matches_across_whitespace() {
    let p = "Rain fell.\n\n  The  streets are wet.";
    let spans = QuoteIndex::new(p).find_all("The streets are wet");
    assert_eq!(spans.len(), 1);
    assert_eq!(&p[spans[0].start..spans[0].end], "The  streets are wet");
}

#[test]
fn markup_must_be_copied() {
    let idx = QuoteIndex::new("This is **essential** here.");
    assert!(idx.contains("**essential**"));
    assert!(!idx.contains("This is essential here"));
}

#[test]
fn quote_does_not_match_inside_a_word() {
    let p = "This is also worth noting here. Rain falls, so streets get wet.";
    let spans = QuoteIndex::new(p).find_all("so");
    assert_eq!(spans.len(), 1, "{spans:?}");
    assert_eq!(&p[spans[0].start..spans[0].end], "so");
    assert!(spans[0].start > p.find("Rain").unwrap());
}

#[test]
fn missing_place_fails_with_quote_in_message() {
    let errs = errors_of("Rain fell.", tree(vec![claim("C1", &["snow fell"])], vec![], &["C1"]));
    assert!(
        errs.iter().any(|e| e.contains("snow fell") && e.contains("character for character")),
        "{errs:?}"
    );
}

#[test]
fn duplicate_places_take_distinct_occurrences() {
    let p = "Z holds. A is true, so Z holds.";
    let idx = QuoteIndex::new(p);
    let ok = check_tree(&idx, tree(vec![claim("C1", &["Z holds", "Z holds"])], vec![], &["C1"]))
        .expect("two places, two occurrences");
    let spans = &ok.claim_spans["C1"];
    assert_eq!(spans.len(), 2);
    assert_ne!(spans[0], spans[1]);

    let errs = errors_of(p, tree(vec![claim("C1", &["Z holds", "Z holds", "Z holds"])], vec![], &["C1"]));
    assert!(!errs.is_empty());
}

#[test]
fn two_claim_loop_is_rejected() {
    let t = tree(
        vec![claim("C1", &["A holds"]), claim("C2", &["B holds"])],
        vec![inf("I1", &["C1"], "C2"), inf("I2", &["C2"], "C1")],
        &["C2"],
    );
    let errs = errors_of(ABC, t);
    assert!(errs.iter().any(|e| e.contains("loop")), "{errs:?}");
}

#[test]
fn self_loop_is_rejected() {
    let t = tree(vec![claim("C1", &["A holds"])], vec![inf("I1", &["C1"], "C1")], &["C1"]);
    let errs = errors_of(ABC, t);
    assert!(errs.iter().any(|e| e.contains("loop")), "{errs:?}");
}

#[test]
fn merging_trees_pass() {
    let t = tree(
        vec![
            claim("X", &["X holds"]),
            claim("Y", &["Y holds"]),
            claim("Q", &["Q holds"]),
            claim("Z", &["Z holds"]),
        ],
        vec![inf("I1", &["X"], "Q"), inf("I2", &["Y"], "Q"), inf("I3", &["Q"], "Z")],
        &["Z"],
    );
    assert!(check_tree(&QuoteIndex::new(ABC), t).is_ok());
}

#[test]
fn unknown_claim_reference_fails() {
    let t = tree(vec![claim("C1", &["A holds"])], vec![inf("I1", &["C9"], "C1")], &["C1"]);
    let errs = errors_of(ABC, t);
    assert!(errs.iter().any(|e| e.contains("C9")), "{errs:?}");
}

#[test]
fn claim_without_place_or_origin_fails() {
    let errs = errors_of(ABC, tree(vec![claim("C1", &[])], vec![], &["C1"]));
    assert!(errs.iter().any(|e| e.contains("no place and no origin")), "{errs:?}");
}

#[test]
fn added_claim_without_place_passes() {
    let mut added = claim("C2", &[]);
    added.origin = Some(Origin { round: 2, critique: "K1".into() });
    let t = tree(
        vec![claim("C1", &["A holds"]), added, claim("C3", &["C holds"])],
        vec![inf("I1", &["C1", "C2"], "C3")],
        &["C3"],
    );
    assert!(check_tree(&QuoteIndex::new(ABC), t).is_ok());
}

#[test]
fn no_conclusion_fails() {
    let errs = errors_of(ABC, tree(vec![claim("C1", &["A holds"])], vec![], &[]));
    assert!(errs.iter().any(|e| e.contains("no conclusion")), "{errs:?}");
}
