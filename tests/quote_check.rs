use sedcontra::check::QuoteIndex;

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

