//! Scoring seam. Matching code asks a `Scorer` for P(candidate | context); the n-gram scorer from the
//! reference is the default. A neural re-ranker can implement the same trait later (see models/README.md).
use crate::model::Model;

pub trait Scorer: Send + Sync {
    /// `context`: previous words of the current sentence, most recent last; empty = start of sentence.
    /// `candidates`: word ids. Returns one score per candidate, higher is better.
    fn score(&self, model: &Model, context: &[&str], candidates: &[u32]) -> Vec<f64>;
}

/// P = lambda * bigram + (1 - lambda) * unigram, trusting the bigram more when the context was seen often.
pub struct NgramScorer;

impl Scorer for NgramScorer {
    fn score(&self, model: &Model, context: &[&str], candidates: &[u32]) -> Vec<f64> {
        let prev = context.last().copied().unwrap_or("^");
        let row = model.row(prev);
        candidates.iter().map(|&w| model.prob(w, row.as_ref())).collect()
    }
}
