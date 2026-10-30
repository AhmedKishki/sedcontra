//! Finds quotes in the proposal word for word, with whitespace normalised.

use crate::model::Span;

pub struct QuoteIndex {
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
        QuoteIndex { normalised, starts, ends }
    }

    /// Every non-overlapping occurrence of `quote`, in order. An empty quote finds nothing.
    pub fn find_all(&self, quote: &str) -> Vec<Span> {
        let needle = normalise(quote);
        if needle.is_empty() {
            return Vec::new();
        }
        self.normalised
            .match_indices(&needle)
            .map(|(i, m)| Span { start: self.starts[i], end: self.ends[i + m.len() - 1] })
            .collect()
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
