use std::collections::{BTreeMap, BTreeSet};

use sedcontra::check::{QuoteIndex, check_adversary_turn, check_advocate_turn, check_revision, check_tree};
use sedcontra::model::{AdversaryTurn, AdvocateTurn, Answer, CheckedTree, Claim, Critique, Inference, Origin, Tree};

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
        reported: false,
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

const CLAIMS: &str = r#"{"id": "C1", "text": "A", "places": ["A holds"]},
    {"id": "C2", "text": "B", "places": ["B holds"]},
    {"id": "C3", "text": "C", "places": ["C holds"]}"#;

const ADDED_C9: &str = r#"{"id": "C9", "text": "hidden", "places": [], "origin": {"round": 2, "critique": "K1"}}"#;

fn base() -> CheckedTree {
    let t = tree(
        vec![claim("C1", &["A holds"]), claim("C2", &["B holds"]), claim("C3", &["C holds"])],
        vec![inf("I1", &["C1", "C2"], "C3")],
        &["C3"],
    );
    check_tree(&QuoteIndex::new(ABC), t).expect("the base tree passes the checks")
}

fn critique(json: &str) -> Critique {
    serde_json::from_str(json).unwrap()
}

fn advocate_errors(turn: &str, round: u32, expected: &[Critique], previous: Option<&CheckedTree>) -> Vec<String> {
    let turn: AdvocateTurn = serde_json::from_str(turn).unwrap();
    let expected: BTreeMap<String, Critique> = expected.iter().map(|c| (c.id.clone(), c.clone())).collect();
    check_advocate_turn(&QuoteIndex::new(ABC), turn, round, &expected, previous).err().unwrap_or_default()
}

fn adversary_errors(turn: &str, tree: &CheckedTree, conceded: &[(&str, &str)]) -> Vec<String> {
    let turn: AdversaryTurn = serde_json::from_str(turn).unwrap();
    let used: BTreeSet<String> = ["K1", "K2"].iter().map(|s| s.to_string()).collect();
    let conceded: BTreeMap<String, String> = conceded.iter().map(|(k, i)| (k.to_string(), i.to_string())).collect();
    check_adversary_turn(&QuoteIndex::new(ABC), &turn, tree, &BTreeSet::new(), &used, &conceded).err().unwrap_or_default()
}

fn round_two(answer: &str, claims: &str, premises: &str) -> String {
    format!(
        r#"{{"answers": [{answer}], "tree": {{"claims": [{claims}],
            "inferences": [{{"id": "I1", "premises": {premises}, "conclusion": "C3"}}], "conclusions": ["C3"]}}}}"#
    )
}

const GAP_K1: &str = r#"{"id": "K1", "kind": "gap", "inference": "I1", "point": "misses Q", "support": "A and B hold, Q fails, C fails"}"#;

#[test]
fn inference_origin_in_first_tree_fails() {
    let turn = format!(
        r#"{{"answers": [], "tree": {{"claims": [{CLAIMS}],
            "inferences": [{{"id": "I1", "premises": ["C1", "C2"], "conclusion": "C3", "origin": {{"round": 1, "critique": "K1"}}}}],
            "conclusions": ["C3"]}}}}"#
    );
    let errs = advocate_errors(&turn, 1, &[], None);
    assert!(errs.iter().any(|e| e.contains("inference I1 has an origin, but the first tree adds nothing")), "{errs:?}");
}

#[test]
fn only_gap_or_circular_can_be_conceded() {
    let k1 = critique(r#"{"id": "K1", "kind": "superfluous", "inference": "I1", "premise": "C2", "point": "p", "support": "s"}"#);
    let turn = round_two(r#"{"critique": "K1", "kind": "concede", "reasons": "r"}"#, CLAIMS, r#"["C1", "C2"]"#);
    let errs = advocate_errors(&turn, 2, &[k1], Some(&base()));
    assert!(errs.iter().any(|e| e.contains("only a gap or circular critique can be conceded")), "{errs:?}");
}

#[test]
fn circular_critique_of_stated_premise_cannot_be_accepted() {
    let k1 = critique(r#"{"id": "K1", "kind": "circular", "inference": "I1", "premise": "C2", "point": "p", "support": "s"}"#);
    let turn = round_two(r#"{"critique": "K1", "kind": "accept", "reasons": "r"}"#, CLAIMS, r#"["C1"]"#);
    let errs = advocate_errors(&turn, 2, &[k1], Some(&base()));
    assert!(errs.iter().any(|e| e.contains("a circular critique of a stated premise cannot be accepted")), "{errs:?}");
}

#[test]
fn stated_rejection_needs_named_premises_in_the_inference() {
    let reject = |premises: &str| {
        format!(r#"{{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "r", "quotes": ["A holds"], "premises": {premises}}}"#)
    };
    let errs = advocate_errors(&round_two(&reject("[]"), CLAIMS, r#"["C1", "C2"]"#), 2, &[critique(GAP_K1)], Some(&base()));
    assert!(errs.iter().any(|e| e.contains("names no claims in premises")), "{errs:?}");

    let errs = advocate_errors(&round_two(&reject(r#"["C1"]"#), CLAIMS, r#"["C1", "C2"]"#), 2, &[critique(GAP_K1)], Some(&base()));
    assert!(errs.is_empty(), "{errs:?}");

    let claims = format!("{CLAIMS}, {ADDED_C9}");
    let errs = advocate_errors(&round_two(&reject(r#"["C9"]"#), &claims, r#"["C1", "C2", "C9"]"#), 2, &[critique(GAP_K1)], Some(&base()));
    assert!(errs.iter().any(|e| e.contains("C9 is not a stated claim")), "{errs:?}");
}

#[test]
fn rejection_premise_errors_arrive_with_tree_errors() {
    let claims = r#"{"id": "C1", "text": "A", "places": ["A fails"]},
        {"id": "C2", "text": "B", "places": ["B holds"]},
        {"id": "C3", "text": "C", "places": ["C holds"]}"#;
    let answer = r#"{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "r", "quotes": ["A holds"], "premises": ["C3"]}"#;
    let errs = advocate_errors(&round_two(answer, claims, r#"["C1", "C2"]"#), 2, &[critique(GAP_K1)], Some(&base()));
    assert!(errs.iter().any(|e| e.contains("does not occur in the proposal")), "{errs:?}");
    assert!(errs.iter().any(|e| e.contains("C3 is not a premise of I1")), "{errs:?}");
}

#[test]
fn stated_rejection_quote_must_lie_in_a_named_place() {
    let answer = r#"{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "r", "quotes": ["B holds"], "premises": ["C1"]}"#;
    let errs = advocate_errors(&round_two(answer, CLAIMS, r#"["C1", "C2"]"#), 2, &[critique(GAP_K1)], Some(&base()));
    assert!(errs.iter().any(|e| e.contains("lies within no place of a named claim")), "{errs:?}");
}

#[test]
fn critique_on_reported_inference_is_refused() {
    let mut t = base();
    t.tree.inferences[0].reported = true;
    let turn = r#"{"critiques": [{"id": "K10", "kind": "gap", "inference": "I1", "point": "p", "support": "s"}], "satisfied": false}"#;
    let errs = adversary_errors(turn, &t, &[]);
    assert!(errs.iter().any(|e| e.contains("takes only reading critiques")), "{errs:?}");
}

#[test]
fn critique_on_conceded_inference_is_refused() {
    let turn = r#"{"critiques": [{"id": "K10", "kind": "gap", "inference": "I1", "point": "p", "support": "s"}], "satisfied": false}"#;
    let errs = adversary_errors(turn, &base(), &[("K1", "I1")]);
    assert!(errs.iter().any(|e| e.contains("conceded to fail (K1)")), "{errs:?}");
}

#[test]
fn superfluous_that_leaves_only_added_premises_is_refused() {
    let mut c9 = claim("C9", &[]);
    c9.origin = Some(Origin { round: 2, critique: "K1".into() });
    let t = tree(
        vec![claim("C1", &["A holds"]), c9, claim("C3", &["C holds"])],
        vec![inf("I1", &["C1", "C9"], "C3")],
        &["C3"],
    );
    let t = check_tree(&QuoteIndex::new(ABC), t).expect("the tree passes the checks");
    let turn = r#"{"critiques": [{"id": "K10", "kind": "superfluous", "inference": "I1", "premise": "C1", "point": "p", "support": "s"}], "satisfied": false}"#;
    let errs = adversary_errors(turn, &t, &[]);
    assert!(errs.iter().any(|e| e.contains("would leave only premises the advocate added")), "{errs:?}");
}

#[test]
fn reading_critique_without_quotes_is_refused() {
    let turn = r#"{"critiques": [{"id": "K10", "kind": "reading", "of": "C1", "point": "p", "support": "s"}], "satisfied": false}"#;
    let errs = adversary_errors(turn, &base(), &[]);
    assert!(errs.iter().any(|e| e.contains("quotes no place")), "{errs:?}");
}

#[test]
fn response_to_conceded_critique_is_refused() {
    let turn = r#"{"responses": [{"critique": "K1", "action": "drop"}], "satisfied": true}"#;
    let errs = adversary_errors(turn, &base(), &[("K1", "I1")]);
    assert!(errs.iter().any(|e| e.contains("K1 was conceded, so it takes no response")), "{errs:?}");
}

fn errors_with(proposal: &str, t: Tree) -> Vec<String> {
    check_tree(&QuoteIndex::new(proposal), t).err().unwrap_or_default()
}

fn cites(mut inference: Inference, places: &[&str]) -> Inference {
    inference.places = places.iter().map(|p| p.to_string()).collect();
    inference
}

#[test]
fn repeated_conclusion_words_do_not_overlap() {
    let p = "If it rains, the streets get wet. It rains. Therefore the streets get wet.";
    let t = tree(
        vec![
            claim("C1", &["If it rains, the streets get wet"]),
            claim("C2", &["It rains"]),
            claim("C3", &["the streets get wet"]),
        ],
        vec![cites(inf("I1", &["C1", "C2"], "C3"), &["Therefore"])],
        &["C3"],
    );
    assert_eq!(errors_with(p, t), Vec::<String>::new());
}

#[test]
fn premise_and_conclusion_sharing_words_fail() {
    let t = tree(
        vec![claim("C1", &["A holds because B holds"]), claim("C2", &["B holds"])],
        vec![cites(inf("I1", &["C2"], "C1"), &["because"])],
        &["C1"],
    );
    let errs = errors_with("A holds because B holds.", t);
    assert!(errs.iter().any(|e| e.contains("quote the same words")), "{errs:?}");
}

#[test]
fn inference_on_added_premises_only_fails() {
    let mut c9 = claim("C9", &[]);
    c9.origin = Some(Origin { round: 2, critique: "K1".into() });
    let t = tree(vec![claim("C1", &["A holds"]), c9, claim("C3", &["C holds"])], vec![inf("I1", &["C9"], "C3")], &["C3"]);
    let errs = errors_with(ABC, t);
    assert!(errs.iter().any(|e| e.contains("rests only on premises the advocate added")), "{errs:?}");
}

#[test]
fn all_failures_reported_in_one_pass() {
    let p = "A holds because B holds.";
    let t = tree(
        vec![claim("C1", &["A holds because B holds"]), claim("C2", &["B holds"]), claim("C3", &["Z holds"])],
        vec![inf("I1", &["C2"], "C1"), inf("I2", &["C7"], "C3")],
        &["C1"],
    );
    let errs = errors_of(p, t);
    assert!(errs.iter().any(|e| e.contains("quote the same words")), "{errs:?}");
    assert!(errs.iter().any(|e| e.contains("does not occur in the proposal")), "{errs:?}");
    assert!(errs.iter().any(|e| e.contains("unknown claim C7")), "{errs:?}");
}

fn added(id: &str, critique: &str) -> Claim {
    let mut c = claim(id, &[]);
    c.origin = Some(Origin { round: 2, critique: critique.into() });
    c
}

fn answer(critique: &str, kind: &str) -> Answer {
    serde_json::from_str(&format!(r#"{{"critique": "{critique}", "kind": "{kind}", "reasons": "r"}}"#)).unwrap()
}

/// C1 and C2 support C3 through I1, and C4 supports C1 through I2.
fn before(extra: Vec<Claim>, i1: &[&str]) -> Tree {
    let mut claims = vec![claim("C1", &["A holds"]), claim("C2", &["B holds"]), claim("C3", &["C holds"]), claim("C4", &["Q holds"])];
    claims.extend(extra);
    tree(claims, vec![inf("I1", i1, "C3"), inf("I2", &["C4"], "C1")], &["C3"])
}

fn revision_errors(previous: &Tree, revised: &Tree, answered: &[(&Critique, &Answer)]) -> Vec<String> {
    check_revision(previous, revised, 2, answered)
}

fn superfluous(id: &str, premise: &str) -> Critique {
    critique(&format!(r#"{{"id": "{id}", "kind": "superfluous", "inference": "I1", "premise": "{premise}", "point": "p", "support": "s"}}"#))
}

#[test]
fn collateral_premise_removal_is_refused() {
    let previous = before(vec![added("C9", "K0")], &["C1", "C2", "C9"]);
    let mut revised = previous.clone();
    revised.inferences[0].premises = vec!["C9".into()];
    let (k1, accept) = (superfluous("K1", "C1"), answer("K1", "accept"));
    let errs = revision_errors(&previous, &revised, &[(&k1, &accept)]);
    assert!(errs.iter().any(|e| e.contains("C2 left I1")), "{errs:?}");
    assert!(!errs.iter().any(|e| e.contains("C1 left I1")), "{errs:?}");
}

#[test]
fn superfluous_removal_of_named_premise_passes() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = previous.clone();
    revised.inferences[0].premises = vec!["C1".into()];
    let (k1, accept) = (superfluous("K1", "C2"), answer("K1", "accept"));
    assert_eq!(revision_errors(&previous, &revised, &[(&k1, &accept)]), Vec::<String>::new());
}

#[test]
fn inference_removal_needs_a_critique_that_touches_it() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = previous.clone();
    revised.inferences.clear();
    let (k1, adapt) = (critique(GAP_K1), answer("K1", "adapt"));
    let errs = revision_errors(&previous, &revised, &[(&k1, &adapt)]);
    assert!(errs.iter().any(|e| e.contains("inference I2 leaves the tree without a critique that touches it")), "{errs:?}");
    assert!(!errs.iter().any(|e| e.contains("inference I1 leaves")), "{errs:?}");
}

#[test]
fn inference_removal_in_answer_to_its_gap_passes() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = previous.clone();
    revised.inferences.remove(0);
    let (k1, adapt) = (critique(GAP_K1), answer("K1", "adapt"));
    assert_eq!(revision_errors(&previous, &revised, &[(&k1, &adapt)]), Vec::<String>::new());
}

#[test]
fn conceded_inference_must_stay() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = previous.clone();
    revised.inferences.remove(0);
    let (k1, concede) = (critique(GAP_K1), answer("K1", "concede"));
    let errs = revision_errors(&previous, &revised, &[(&k1, &concede)]);
    assert!(errs.iter().any(|e| e.contains("was conceded in this turn")), "{errs:?}");
}

#[test]
fn stated_claim_deletion_needs_a_reading_critique() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = previous.clone();
    revised.inferences.remove(1);
    revised.claims.retain(|c| c.id != "C4");
    let k1 = critique(r#"{"id": "K1", "kind": "gap", "inference": "I2", "point": "p", "support": "s"}"#);
    let adapt = answer("K1", "adapt");
    let errs = revision_errors(&previous, &revised, &[(&k1, &adapt)]);
    assert!(errs.iter().any(|e| e.contains("claim C4 leaves the tree, but the proposal states it")), "{errs:?}");
}

#[test]
fn reported_flag_change_needs_a_reading_critique() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = previous.clone();
    revised.inferences[0].reported = true;
    let (k1, adapt) = (critique(GAP_K1), answer("K1", "adapt"));
    let errs = revision_errors(&previous, &revised, &[(&k1, &adapt)]);
    assert!(errs.iter().any(|e| e.contains("I1 changed its reported flag without a reading critique")), "{errs:?}");
}

#[test]
fn new_inference_needs_an_origin() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = previous.clone();
    revised.inferences.push(inf("I3", &["C2"], "C4"));
    let (k1, adapt) = (critique(GAP_K1), answer("K1", "adapt"));
    let errs = revision_errors(&previous, &revised, &[(&k1, &adapt)]);
    assert!(errs.iter().any(|e| e.contains("inference I3 is new, so it needs an origin")), "{errs:?}");
}

#[test]
fn patch_in_an_untouched_inference_is_refused() {
    let previous = before(vec![], &["C1", "C2"]);
    let mut revised = before(vec![added("C9", "K1")], &["C1", "C2"]);
    revised.inferences[1].premises.push("C9".into());
    let (k1, accept) = (critique(GAP_K1), answer("K1", "accept"));
    let errs = revision_errors(&previous, &revised, &[(&k1, &accept)]);
    assert!(errs.iter().any(|e| e.contains("claim C9 is not a premise of an inference its origin critique touches")), "{errs:?}");
}

#[test]
fn accepted_gap_must_add_a_premise() {
    let previous = before(vec![], &["C1", "C2"]);
    let (k1, accept) = (critique(GAP_K1), answer("K1", "accept"));
    let errs = revision_errors(&previous, &previous.clone(), &[(&k1, &accept)]);
    assert!(errs.iter().any(|e| e.contains("accepting a gap gives I1 an added premise")), "{errs:?}");
}

#[test]
fn accepted_superfluous_must_remove_the_premise() {
    let previous = before(vec![], &["C1", "C2"]);
    let (k1, accept) = (superfluous("K1", "C2"), answer("K1", "accept"));
    let errs = revision_errors(&previous, &previous.clone(), &[(&k1, &accept)]);
    assert!(errs.iter().any(|e| e.contains("removes C2 from I1")), "{errs:?}");
}

#[test]
fn conceded_circular_removes_the_added_premise() {
    let previous = before(vec![added("C9", "K0")], &["C1", "C9"]);
    let k1 = critique(r#"{"id": "K1", "kind": "circular", "inference": "I1", "premise": "C9", "point": "p", "support": "s"}"#);
    let concede = answer("K1", "concede");
    let errs = revision_errors(&previous, &previous.clone(), &[(&k1, &concede)]);
    assert!(errs.iter().any(|e| e.contains("conceding a circular critique of C9, which you added, removes it from I1")), "{errs:?}");

    let mut revised = previous.clone();
    revised.inferences[0].premises = vec!["C1".into()];
    assert_eq!(revision_errors(&previous, &revised, &[(&k1, &concede)]), Vec::<String>::new());
}

#[test]
fn two_critiques_on_one_inference_in_one_turn() {
    let previous = before(vec![], &["C1", "C2"]);
    let revised = before(vec![added("C9", "K1")], &["C1", "C9"]);
    let (k1, k2) = (critique(GAP_K1), superfluous("K2", "C2"));
    let (accept1, accept2) = (answer("K1", "accept"), answer("K2", "accept"));
    assert_eq!(revision_errors(&previous, &revised, &[(&k1, &accept1), (&k2, &accept2)]), Vec::<String>::new());
}
