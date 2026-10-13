//! Phase 4 seams: model manifest, user dictionary interface, and (feature `neural`) the async re-rank pipeline.
mod common;
use common::root;
use khcore::manifest::ModelManifest;
use khcore::userdict::{NullUserDictionary, UserDictionary};

#[test]
fn example_manifest_parses_with_defaults() {
    let m = ModelManifest::from_json(&std::fs::read_to_string(root().join("models/example/manifest.json")).unwrap())
        .unwrap();
    assert_eq!((m.name.as_str(), m.version.as_str(), m.kind.as_str()), ("khmer-lm-small", "0.0.0", "onnx-causal-lm"));
    assert_eq!((m.rerank_top_k, m.context_tokens), (20, 32));
    let minimal = r#"{"name":"x","version":"1.2.3","kind":"onnx-causal-lm","file":"m.onnx","size_bytes":5,"tokenizer":"t.model","context_tokens":8}"#;
    let d = ModelManifest::from_json(minimal).unwrap();
    assert_eq!((d.rerank_top_k, d.blend_alpha), (20, 0.5));
}

#[test]
fn bad_manifests_are_rejected() {
    let ok = |patch: &str| {
        format!(
            r#"{{"name":"x","version":"1.2.3","kind":"onnx-causal-lm","file":"m.onnx","size_bytes":5,"tokenizer":"t.model","context_tokens":8{patch}}}"#
        )
    };
    assert!(ModelManifest::from_json(&ok("")).is_ok());
    for (patch, why) in [
        (r#","extra":1"#, "unknown field"),
        (r#","blend_alpha":1.5"#, "alpha out of range"),
        (r#","rerank_top_k":0"#, "top_k out of range"),
        (r#","sha256":"XYZ""#, "bad hash"),
    ] {
        assert!(ModelManifest::from_json(&ok(patch)).is_err(), "{why}");
    }
    for bad in [
        r#"{"name":"","version":"1.2.3","kind":"onnx-causal-lm","file":"m","size_bytes":5,"tokenizer":"t","context_tokens":8}"#,
        r#"{"name":"x","version":"1.2","kind":"onnx-causal-lm","file":"m","size_bytes":5,"tokenizer":"t","context_tokens":8}"#,
        r#"{"name":"x","version":"1.2.3","kind":"pickle","file":"m","size_bytes":5,"tokenizer":"t","context_tokens":8}"#,
        r#"{"name":"x","version":"1.2.3","kind":"onnx-causal-lm","file":"../m","size_bytes":5,"tokenizer":"t","context_tokens":8}"#,
        r#"{"name":"x","version":"1.2.3","kind":"onnx-causal-lm","file":"/etc/m","size_bytes":5,"tokenizer":"t","context_tokens":8}"#,
        r#"{"name":"x","version":"1.2.3","kind":"onnx-causal-lm","file":"m","size_bytes":0,"tokenizer":"t","context_tokens":8}"#,
        r#"{"name":"x","version":"1.2.3","kind":"onnx-causal-lm","file":"m","size_bytes":5,"tokenizer":"t","context_tokens":0}"#,
        "not json",
    ] {
        assert!(ModelManifest::from_json(bad).is_err(), "{bad}");
    }
}

#[test]
fn validate_checks_the_files_next_to_the_manifest() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("manifest-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.onnx"), b"12345").unwrap();
    std::fs::write(dir.join("t.model"), b"tok").unwrap();
    let json = r#"{"name":"x","version":"1.2.3","kind":"onnx-causal-lm","file":"m.onnx","size_bytes":5,"tokenizer":"t.model","context_tokens":8}"#;
    std::fs::write(dir.join("manifest.json"), json).unwrap();
    assert!(ModelManifest::load(&dir).is_ok());
    std::fs::write(dir.join("m.onnx"), b"123").unwrap();
    assert!(ModelManifest::load(&dir).unwrap_err().contains("bytes"));
    std::fs::remove_file(dir.join("t.model")).unwrap();
    std::fs::write(dir.join("m.onnx"), b"12345").unwrap();
    assert!(ModelManifest::load(&dir).is_err());
    // the placeholder manifest in the repo has no model files, so it must not validate
    assert!(ModelManifest::load(&root().join("models/example")).is_err());
}

#[test]
fn null_user_dictionary_learns_nothing() {
    let mut d = NullUserDictionary;
    d.record_accept("^", "ខ្ញុំ");
    assert_eq!(d.boost("^", "ខ្ញុំ"), 0.0);
    d.clear();
}

#[cfg(feature = "neural")]
mod neural {
    use khcore::neural::*;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// scores = candidate length, after an optional pause
    struct Slow(Duration, Arc<Mutex<Vec<u64>>>);
    impl Reranker for Slow {
        fn rerank(&mut self, context: &[String], candidates: &[String]) -> Vec<f32> {
            self.1.lock().unwrap().push(context.len() as u64);
            std::thread::sleep(self.0);
            candidates.iter().map(|c| c.chars().count() as f32).collect()
        }
    }
    fn req(id: u64, n: usize) -> RefineRequest {
        RefineRequest {
            id, context: vec!["a".into(); n], candidates: vec!["ក".into(), "កក".into(), "កកក".into()]
        }
    }

    #[test]
    fn submit_never_blocks_and_a_result_arrives() {
        let seen = Arc::new(Mutex::new(vec![]));
        let r = AsyncRefiner::spawn(Slow(Duration::from_millis(150), seen.clone()));
        let t = Instant::now();
        r.submit(req(1, 1));
        assert!(t.elapsed() < Duration::from_millis(50), "submit must return at once");
        assert!(r.poll().is_none(), "the model has not answered yet");
        let res = r.wait(Duration::from_secs(5)).expect("result");
        assert_eq!((res.id, res.scores), (1, vec![1.0, 2.0, 3.0]));
    }

    #[test]
    fn a_newer_request_supersedes_queued_older_ones() {
        let seen = Arc::new(Mutex::new(vec![]));
        let r = AsyncRefiner::spawn(Slow(Duration::from_millis(200), seen.clone()));
        r.submit(req(1, 1)); // starts running at once
        std::thread::sleep(Duration::from_millis(50));
        for i in 2..=6 {
            r.submit(req(i, i as usize)); // piles up behind request 1
        }
        let mut last = None;
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            if let Some(x) = r.wait(Duration::from_millis(100)) {
                let id = x.id;
                last = Some(x);
                if id == 6 {
                    break;
                }
            }
        }
        assert_eq!(last.unwrap().id, 6, "the newest request is answered");
        // the model ran request 1 and the newest one; requests 2..=5 were skipped
        assert_eq!(*seen.lock().unwrap(), vec![1, 6]);
    }

    #[test]
    fn dropping_the_refiner_stops_the_worker() {
        let seen = Arc::new(Mutex::new(vec![]));
        let r = AsyncRefiner::spawn(Slow(Duration::from_millis(10), seen));
        r.submit(req(1, 1));
        drop(r); // joins the worker: must not hang
    }

    #[test]
    fn blend_cannot_make_things_worse_than_ngram_at_alpha_zero() {
        let ngram = [0.5, 0.3, 0.2];
        let model = [0.0f32, 1.0, 5.0];
        assert_eq!(order_by(&blend(&ngram, &model, 0.0)), vec![0, 1, 2]);
        assert_eq!(order_by(&blend(&ngram, &model, 1.0)), vec![2, 1, 0]);
        // a constant model score changes nothing
        assert_eq!(order_by(&blend(&ngram, &[1.0, 1.0, 1.0], 0.7)), vec![0, 1, 2]);
        // all-equal n-gram scores: the model decides
        assert_eq!(order_by(&blend(&[1.0, 1.0, 1.0], &model, 0.5)), vec![2, 1, 0]);
    }
}
