use sedcontra::analyze::{Analyzer, ConclusionSupport, Finding, Restatement, Stubs, Uncovered, Unsupported, conclusion_support};
use sedcontra::check::{QuoteIndex, check_tree, unsupported_conclusions};
use sedcontra::model::{CheckedTree, Claim, CritiqueStatus, Inference, LedgerRow, Origin, Tree};

fn claim(id: &str, places: &[&str]) -> Claim {
    Claim {
        id: id.into(),
        text: id.into(),
        places: places.iter().map(|p| p.to_string()).collect(),
        origin: None,
        backed_by_context: false,
    }
}

fn inf(id: &str, premises: &[&str], conclusion: &str, places: &[&str]) -> Inference {
    Inference {
        id: id.into(),
        premises: premises.iter().map(|p| p.to_string()).collect(),
        conclusion: conclusion.into(),
        places: places.iter().map(|p| p.to_string()).collect(),
        origin: None,
        reported: false,
    }
}

fn checked(proposal: &str, claims: Vec<Claim>, inferences: Vec<Inference>, conclusions: &[&str]) -> CheckedTree {
    let tree = Tree { claims, inferences, conclusions: conclusions.iter().map(|c| c.to_string()).collect() };
    check_tree(&QuoteIndex::new(proposal), tree).expect("test tree must pass the checks")
}

fn stubs(findings: Vec<Finding>) -> Vec<(Vec<String>, bool)> {
    findings
        .into_iter()
        .filter_map(|f| match f {
            Finding::Stub { claims, prune } => Some((claims, prune)),
            _ => None,
        })
        .collect()
}

const LETTERS: &str = "X holds. Y holds. Q holds. Z holds. P holds. S holds. S1 holds. S2 holds. S3 holds. S4 holds.";

#[test]
fn restatement_reports_every_place() {
    let p = "Z holds. A is true. So Z holds.";
    let t = checked(
        p,
        vec![claim("C1", &["A is true"]), claim("C3", &["Z holds", "Z holds"])],
        vec![inf("I1", &["C1"], "C3", &["So"])],
        &["C3"],
    );
    let findings = Restatement.analyze(p, &t);
    assert_eq!(findings.len(), 1);
    match &findings[0] {
        Finding::Restatement { claim, quotes, spans } => {
            assert_eq!(claim, "C3");
            assert_eq!(quotes.len(), 2);
            assert_eq!(spans.len(), 2);
        }
        other => panic!("unexpected finding {other:?}"),
    }
}

#[test]
fn uncovered_text_respects_minimum() {
    let p = "Indeed, as we all know very well, rain falls. So streets get wet.";
    let t = checked(
        p,
        vec![claim("C1", &["rain falls"]), claim("C2", &["streets get wet"])],
        vec![inf("I1", &["C1"], "C2", &["So"])],
        &["C2"],
    );
    let findings = Uncovered { min_words: 4 }.analyze(p, &t);
    assert_eq!(findings.len(), 1, "{findings:?}");
    match &findings[0] {
        Finding::Uncovered { text, span } => {
            assert_eq!(text, "Indeed, as we all know very well,");
            assert_eq!(&p[span.start..span.end], text);
        }
        other => panic!("unexpected finding {other:?}"),
    }
}

#[test]
fn stubs_split_by_length() {
    let t = checked(
        LETTERS,
        vec![
            claim("X", &["X holds"]),
            claim("Z", &["Z holds"]),
            claim("S1", &["S1 holds"]),
            claim("S2", &["S2 holds"]),
            claim("S3", &["S3 holds"]),
            claim("S4", &["S4 holds"]),
        ],
        vec![
            inf("I1", &["X"], "Z", &[]),
            inf("I2", &["S2"], "S3", &[]),
            inf("I3", &["S3"], "S4", &[]),
        ],
        &["Z"],
    );
    let found = stubs(Stubs { max_len: 2 }.analyze(LETTERS, &t));
    assert_eq!(
        found,
        vec![
            (vec!["S1".to_string()], true),
            (vec!["S2".to_string(), "S3".to_string(), "S4".to_string()], false),
        ]
    );
}

#[test]
fn merged_trees_have_no_stubs() {
    let t = checked(
        LETTERS,
        vec![claim("X", &["X holds"]), claim("Y", &["Y holds"]), claim("Q", &["Q holds"]), claim("Z", &["Z holds"])],
        vec![inf("I1", &["X"], "Q", &[]), inf("I2", &["Y"], "Q", &[]), inf("I3", &["Q"], "Z", &[])],
        &["Z"],
    );
    assert!(stubs(Stubs { max_len: 2 }.analyze(LETTERS, &t)).is_empty());
}

#[test]
fn unused_conclusion_of_argument_is_stub() {
    let t = checked(
        LETTERS,
        vec![claim("P", &["P holds"]), claim("Z", &["Z holds"]), claim("S", &["S holds"])],
        vec![inf("I1", &["P"], "Z", &[]), inf("I2", &["P"], "S", &[])],
        &["Z"],
    );
    assert_eq!(stubs(Stubs { max_len: 2 }.analyze(LETTERS, &t)), vec![(vec!["S".to_string()], true)]);
}

fn reported(mut inference: Inference) -> Inference {
    inference.reported = true;
    inference
}

fn plain(claims: Vec<Claim>, inferences: Vec<Inference>, conclusions: &[&str]) -> Tree {
    Tree { claims, inferences, conclusions: conclusions.iter().map(|c| c.to_string()).collect() }
}

#[test]
fn unsupported_conclusion_ignores_reported_support() {
    let t = plain(
        vec![claim("X", &[]), claim("Z1", &[]), claim("Z2", &[])],
        vec![reported(inf("I1", &["X"], "Z1", &[])), inf("I2", &["X"], "Z2", &[])],
        &["Z1", "Z2"],
    );
    assert_eq!(unsupported_conclusions(&t), vec!["Z1".to_string()]);
}

#[test]
fn unsupported_analyzer_reports_conclusion_without_inference() {
    let t = checked(LETTERS, vec![claim("X", &["X holds"]), claim("Z", &["Z holds"])], vec![], &["Z"]);
    assert_eq!(Unsupported.analyze(LETTERS, &t), vec![Finding::Unsupported { claim: "Z".into() }]);
}

fn row(critique: &str, status: CritiqueStatus) -> LedgerRow {
    LedgerRow { critique: serde_json::from_str(critique).unwrap(), raised_in: 1, status }
}

fn gap_on(id: &str, inference: &str) -> String {
    format!(r#"{{"id": "{id}", "kind": "gap", "inference": "{inference}", "point": "p", "support": "s"}}"#)
}

fn support_of(t: &Tree, ledger: &[LedgerRow]) -> ConclusionSupport {
    conclusion_support(t, ledger).into_iter().next().expect("one conclusion")
}

#[test]
fn support_through_conceded_inference_is_not_intact() {
    let t = plain(vec![claim("A", &[]), claim("Z", &[])], vec![inf("I1", &["A"], "Z", &[])], &["Z"]);
    let s = support_of(&t, &[row(&gap_on("K1", "I1"), CritiqueStatus::Conceded)]);
    assert!(s.supported);
    assert!(!s.intact);
    assert_eq!(s.conceded, vec!["I1".to_string()]);
}

#[test]
fn alternative_line_keeps_support_intact() {
    let t = plain(
        vec![claim("A", &[]), claim("B", &[]), claim("Z", &[])],
        vec![inf("I1", &["A"], "Z", &[]), inf("I2", &["B"], "Z", &[])],
        &["Z"],
    );
    let s = support_of(&t, &[row(&gap_on("K1", "I1"), CritiqueStatus::Conceded)]);
    assert!(s.intact);
    assert_eq!(s.conceded, vec!["I1".to_string()]);
}

#[test]
fn reported_inference_is_not_support() {
    let t = plain(vec![claim("A", &[]), claim("Z", &[])], vec![reported(inf("I1", &["A"], "Z", &[]))], &["Z"]);
    let s = support_of(&t, &[]);
    assert!(!s.supported);
    assert!(!s.intact);
}

#[test]
fn support_collects_open_unanswered_and_added() {
    let mut c9 = claim("C9", &[]);
    c9.origin = Some(Origin { round: 2, critique: "K0".into() });
    let t = plain(
        vec![claim("A", &[]), c9, claim("M", &[]), claim("Z", &[]), claim("Q", &[]), claim("R", &[])],
        vec![inf("I1", &["A", "C9"], "M", &[]), inf("I2", &["M"], "Z", &[]), inf("I9", &["Q"], "R", &[])],
        &["Z"],
    );
    let ledger = [
        row(&gap_on("K1", "I1"), CritiqueStatus::Open),
        row(r#"{"id": "K2", "kind": "reading", "of": "A", "point": "p", "support": "s", "quotes": ["A"]}"#, CritiqueStatus::Unanswered),
        row(&gap_on("K3", "I9"), CritiqueStatus::Unanswered),
    ];
    let s = support_of(&t, &ledger);
    assert_eq!(s.added, vec!["C9".to_string()]);
    assert_eq!(s.open, vec!["K1".to_string()]);
    assert_eq!(s.unanswered, vec!["K2".to_string()]);
    assert!(s.intact);
}
