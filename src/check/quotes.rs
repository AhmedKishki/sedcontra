//! Finds quotes in the proposal word for word, with whitespace normalised.

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
