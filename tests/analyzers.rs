use cross_examine::analyze::{Analyzer, Finding, Restatement, Stubs, Uncovered};
use cross_examine::check::{QuoteIndex, check_tree};
use cross_examine::model::{CheckedTree, Claim, Inference, Tree};

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
