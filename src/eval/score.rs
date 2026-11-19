//! Scores a finished debate against a case: which critiques land on each planted defect, and the outcome that stands for it.

use serde::Serialize;

use crate::check::{QuoteIndex, Span};
use crate::eval::case::Case;
use crate::model::{CritiqueId, CritiqueStatus, Thread};

/// What the debate did with one planted defect.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Missed,
    Accepted,
    Open,
    Dropped,
}

/// A critique that landed on a defect, or on none, with the status the debate left it in.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Landing {
    pub critique: CritiqueId,
    pub kind: String,
    pub status: CritiqueStatus,
}

#[derive(Serialize, Clone, Debug)]
pub struct DefectScore {
    pub defect: String,
    pub kind: String,
    pub outcome: Outcome,
    pub same_kind: bool,
    pub landed: Vec<Landing>,
}

#[derive(Serialize, Clone, Debug)]
pub struct RunScore {
    pub defects: Vec<DefectScore>,
    pub unplanted: Vec<Landing>,
}

/// Scores the threads of a debate on the proposal of `case`.
///
/// A critique's spans are every occurrence in the proposal of every one of its places. A critique
/// lands on a defect when one of its spans overlaps one of the defect's spans, and two spans
/// overlap when each starts before the other ends, so spans that only touch do not overlap.
///
/// A defect on which no critique lands is `Missed`. Otherwise its outcome is `Accepted` when a
/// landing critique is accepted, else `Open` when one is open, else `Dropped`. An accepted
/// critique at the defect means the debate confirmed a failure there, and an open one means the
/// debate has not ruled the failure out, so the strongest status among the landings stands for
/// the defect.
///
/// `same_kind` is true when a landing critique names the defect's kind, ignoring spaces at the
/// ends. A critique that lands on no defect is `unplanted`, in thread order, and the score does
/// not judge it. A critique that lands on two defects counts for both.
pub fn score(case: &Case, threads: &[Thread]) -> RunScore {
    let index = QuoteIndex::new(&case.proposal);
    let mut landed: Vec<Vec<Landing>> = vec![Vec::new(); case.defects.len()];
    let mut unplanted = Vec::new();
    for thread in threads {
        let critique = &thread.critique;
        let spans: Vec<Span> = critique.places.iter().flat_map(|place| index.find_all(place)).collect();
        let landing = Landing { critique: critique.id.clone(), kind: critique.kind.clone(), status: thread.status };
        let mut lands = false;
        for (defect, landings) in case.defects.iter().zip(landed.iter_mut()) {
            if defect.spans.iter().any(|d| spans.iter().any(|s| overlap(s, d))) {
                landings.push(landing.clone());
                lands = true;
            }
        }
        if !lands {
            unplanted.push(landing);
        }
    }

    let defects = case
        .defects
        .iter()
        .zip(landed)
        .map(|(defect, landed)| DefectScore {
            defect: defect.id.clone(),
            kind: defect.kind.clone(),
            outcome: outcome(&landed),
            same_kind: landed.iter().any(|l| l.kind.trim() == defect.kind.trim()),
            landed,
        })
        .collect();
    RunScore { defects, unplanted }
}

fn overlap(a: &Span, b: &Span) -> bool {
    a.start < b.end && b.start < a.end
}

fn outcome(landed: &[Landing]) -> Outcome {
    landed
        .iter()
        .map(|landing| match landing.status {
            CritiqueStatus::Dropped => (0, Outcome::Dropped),
            CritiqueStatus::Open => (1, Outcome::Open),
            CritiqueStatus::Accepted => (2, Outcome::Accepted),
        })
        .max_by_key(|(strength, _)| *strength)
        .map_or(Outcome::Missed, |(_, outcome)| outcome)
}
