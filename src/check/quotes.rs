//! Finds quotes in the proposal word for word, with whitespace normalised, and the sentences and
//! words of its text.

use std::collections::BTreeSet;

use crate::model::Span;

pub struct QuoteIndex {
    proposal: String,
    normalised: String,
    /// For every byte of `normalised`, the original byte range of the character
    /// or whitespace run it came from.
    starts: Vec<usize>,
    ends: Vec<usize>,
}

impl QuoteIndex {
    pub fn new(proposal: &str) -> Self {
        let mut normalised = String::with_capacity(proposal.len());
        let mut starts = Vec::with_capacity(proposal.len());
        let mut ends = Vec::with_capacity(proposal.len());
        let mut chars = proposal.char_indices().peekable();
        while let Some((start, c)) = chars.next() {
            if c.is_whitespace() {
                let mut end = start + c.len_utf8();
                while let Some(&(i, next)) = chars.peek() {
                    if !next.is_whitespace() {
                        break;
                    }
                    end = i + next.len_utf8();
                    chars.next();
                }
                normalised.push(' ');
                starts.push(start);
                ends.push(end);
            } else {
                let end = start + c.len_utf8();
                normalised.push(c);
                for _ in 0..c.len_utf8() {
                    starts.push(start);
                    ends.push(end);
                }
            }
        }
        QuoteIndex { proposal: proposal.to_string(), normalised, starts, ends }
    }

    pub fn proposal(&self) -> &str {
        &self.proposal
    }

    /// Every non-overlapping occurrence of `quote`, in order. A quote that starts or ends with a
    /// letter or digit matches only at a word boundary, so "so" does not match inside "also".
    /// An empty quote finds nothing.
    pub fn find_all(&self, quote: &str) -> Vec<Span> {
        let needle = normalise(quote);
        let Some(first) = needle.chars().next() else {
            return Vec::new();
        };
        let last = needle.chars().next_back().unwrap_or(first);
        let text = &self.normalised;
        let bounded = |start: usize, end: usize| {
            let before = text[..start].chars().next_back();
            let after = text[end..].chars().next();
            (!first.is_alphanumeric() || !before.is_some_and(char::is_alphanumeric))
                && (!last.is_alphanumeric() || !after.is_some_and(char::is_alphanumeric))
        };

        let mut spans = Vec::new();
        let mut from = 0;
        while let Some(offset) = text[from..].find(&needle) {
            let start = from + offset;
            let end = start + needle.len();
            if bounded(start, end) {
                spans.push(Span { start: self.starts[start], end: self.ends[end - 1] });
                from = end;
            } else {
                from = start + first.len_utf8();
            }
        }
        spans
    }

    pub fn contains(&self, quote: &str) -> bool {
        !self.find_all(quote).is_empty()
    }
}

/// Collapses every whitespace run to one space and trims the ends.
pub fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The failure message for a quote that does not occur in the proposal.
pub fn missing_quote_message(owner: &str, quote: &str) -> String {
    format!(
        "{owner}: the quote \"{quote}\" does not occur in the proposal; copy it from the proposal character for character, including markup and punctuation"
    )
}

/// Where the first sentence of `text` ends, when another sentence, a paragraph or a list item
/// starts after it within `text`. A sentence ends at ".", "!" or "?" followed by whitespace and
/// a capital letter, so "e.g. the" stays one sentence.
pub fn sentence_break(text: &str) -> Option<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|&(_, c)| c);
    for (k, &(offset, c)) in chars.iter().enumerate() {
        let mut i = k + 1;
        match c {
            '.' | '!' | '?' => {
                while at(i).is_some_and(|c| "\"”’)]".contains(c)) {
                    i += 1;
                }
                let space = i;
                while at(i).is_some_and(char::is_whitespace) {
                    i += 1;
                }
                while i > space && at(i).is_some_and(|c| "\"“‘([".contains(c)) {
                    i += 1;
                }
                if i > space && at(i).is_some_and(char::is_uppercase) {
                    return Some(offset + c.len_utf8());
                }
            }
            '\n' => {
                while at(i).is_some_and(|c| c == ' ' || c == '\t') {
                    i += 1;
                }
                let digits = (i..).take_while(|&j| at(j).is_some_and(|c| c.is_ascii_digit())).count();
                let list_item = match at(i) {
                    Some('-' | '*' | '+') => at(i + 1) == Some(' '),
                    Some(_) if digits > 0 => matches!(at(i + digits), Some('.' | ')')) && at(i + digits + 1) == Some(' '),
                    _ => false,
                };
                if at(i) == Some('\n') || list_item {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// The words of `text` with their spans: runs of letters, digits and apostrophes, lowercased,
/// with ’ read as '.
fn word_spans(text: &str) -> Vec<(Span, String)> {
    let apostrophe = |c: char| c == '\'' || c == '’';
    let mut found = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        match (start, c.is_alphanumeric() || apostrophe(c)) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                let raw = &text[s..i];
                let word = raw.trim_matches(apostrophe);
                if !word.is_empty() {
                    let begin = s + raw.len() - raw.trim_start_matches(apostrophe).len();
                    found.push((Span { start: begin, end: begin + word.len() }, word.replace('’', "'").to_lowercase()));
                }
                start = None;
            }
            _ => {}
        }
    }
    found
}

/// The distinct words of `text`, lowercased, with ’ read as '.
pub fn words(text: &str) -> BTreeSet<String> {
    word_spans(text).into_iter().map(|(_, word)| word).collect()
}

/// Every whole-word occurrence in `text` of one of `listed`, ignoring case.
pub fn find_words(text: &str, listed: &[String]) -> Vec<Span> {
    let listed: BTreeSet<String> = listed.iter().map(|w| w.replace('’', "'").to_lowercase()).collect();
    word_spans(text).into_iter().filter(|(_, word)| listed.contains(word)).map(|(span, _)| span).collect()
}
