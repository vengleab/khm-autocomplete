//! Asynchronous neural re-ranking pipeline (feature `neural`, off by default). Contains no model code and no
//! ONNX dependency: a `Reranker` is whatever scores candidates (ONNX Runtime later, a mock in tests). What is
//! settled here is the part that must be right before a model exists: typing never waits, a newer request
//! supersedes an older one, and the model can only blend with the n-gram scores, never replace them.
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

/// Scores candidate words given the left context. Runs on the worker thread, so it may take a long time.
pub trait Reranker: Send + 'static {
    /// `context`: previous words, most recent last. Returns one score per candidate, higher is better.
    fn rerank(&mut self, context: &[String], candidates: &[String]) -> Vec<f32>;
}

pub struct RefineRequest {
    /// the caller's id for this request (e.g. a keystroke counter); echoed in the result
    pub id: u64,
    pub context: Vec<String>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefineResult {
    pub id: u64,
    pub scores: Vec<f32>,
}

pub struct AsyncRefiner {
    tx: Option<Sender<RefineRequest>>,
    rx: Receiver<RefineResult>,
    worker: Option<JoinHandle<()>>,
}

impl AsyncRefiner {
    pub fn spawn(mut model: impl Reranker) -> AsyncRefiner {
        let (tx, jobs) = channel::<RefineRequest>();
        let (done, rx) = channel::<RefineResult>();
        let worker = std::thread::spawn(move || {
            // wait for a job, then skip ahead to the newest one already queued: stale text is not worth scoring
            while let Ok(mut job) = jobs.recv() {
                while let Ok(newer) = jobs.try_recv() {
                    job = newer;
                }
                let scores = model.rerank(&job.context, &job.candidates);
                if done.send(RefineResult { id: job.id, scores }).is_err() {
                    break;
                }
            }
        });
        AsyncRefiner { tx: Some(tx), rx, worker: Some(worker) }
    }

    /// Queue a request and return immediately.
    pub fn submit(&self, req: RefineRequest) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(req);
        }
    }

    /// The newest finished result, if any (older finished results are discarded). Never blocks.
    pub fn poll(&self) -> Option<RefineResult> {
        let mut last = None;
        while let Ok(r) = self.rx.try_recv() {
            last = Some(r);
        }
        last
    }

    /// Wait up to `timeout` for a result (tests, and shells that tick on a timer).
    pub fn wait(&self, timeout: std::time::Duration) -> Option<RefineResult> {
        self.rx.recv_timeout(timeout).ok()
    }
}

impl Drop for AsyncRefiner {
    fn drop(&mut self) {
        self.tx = None; // closes the queue; the worker finishes its current job and exits
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

/// `alpha * model + (1 - alpha) * ngram`, both min-max normalized to 0..1 first so their scales do not matter.
/// alpha = 0 gives back the n-gram order exactly; a constant score list normalizes to all zeros.
pub fn blend(ngram: &[f64], model: &[f32], alpha: f64) -> Vec<f64> {
    fn norm(v: &[f64]) -> Vec<f64> {
        let (lo, hi) = v.iter().fold((f64::MAX, f64::MIN), |(l, h), &x| (l.min(x), h.max(x)));
        v.iter().map(|&x| if hi > lo { (x - lo) / (hi - lo) } else { 0.0 }).collect()
    }
    let m: Vec<f64> = model.iter().map(|&x| x as f64).collect();
    let (n, m) = (norm(ngram), norm(&m));
    n.iter().zip(&m).map(|(a, b)| (1.0 - alpha) * a + alpha * b).collect()
}

/// Indices that sort `scores` best first; ties keep the original order.
pub fn order_by(scores: &[f64]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.sort_by(|&a, &b| scores[b].partial_cmp(&scores[a]).unwrap_or(std::cmp::Ordering::Equal));
    idx
}
