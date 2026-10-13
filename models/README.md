# Models (planned, not implemented)

The engine ranks candidates with a `Scorer` (`core/src/scorer.rs`). The default is the n-gram scorer from the
reference implementation. This folder is where a small neural language model will live. **No model ships yet, and
the `neural` cargo feature is off by default**, so a default build has no ONNX dependency and no model code.

## Plan

1. **Train offline in Python** on a Khmer corpus (outside this repo): a small causal LM, then export to ONNX and
   quantize to int8 (target: a few tens of MB, well under the 50 MB RAM budget including the n-gram data).
2. **Tokenizer:** a SentencePiece model file ships next to the ONNX file. `manifest.json` names both.
3. **Runtime:** ONNX Runtime through the `ort` crate, behind the `neural` feature (`neural = ["dep:ort"]` when it is
   implemented). Chosen over `tract` for int8 speed; revisit if the binary size is a problem on Windows.
4. **Typing never waits on the model.** The n-gram candidates are shown immediately. The top 20 are sent to a
   worker thread (`core/src/neural.rs`: `AsyncRefiner`); when the model answers, the list is re-ranked and the shell
   is told (the shell asks `Engine::suggest` again or takes the refined order). A newer request supersedes an older one,
   so a fast typist never builds a queue, and a result for stale text is dropped.
5. **Blend, do not replace:** final score = `alpha * model + (1 - alpha) * ngram` on normalized scores, so a bad
   model cannot make the keyboard worse than the n-gram baseline (`neural::blend`).
6. **Everything on the device.** No network, no telemetry. The model file is checked against `sha256` from the manifest
   before it is loaded.

## Manifest

`manifest.json` next to the model files (see `example/manifest.json`, schema in `manifest.schema.json`), parsed by
`khcore::manifest::ModelManifest`:

| field | meaning |
|---|---|
| `name`, `version` | identity; `version` is semver |
| `kind` | `"onnx-causal-lm"` (the only kind so far) |
| `file`, `size_bytes`, `sha256` | the ONNX file, relative to the manifest; size and hash are verified |
| `tokenizer` | the SentencePiece file, relative to the manifest |
| `context_tokens` | how many tokens of left context the model reads |
| `rerank_top_k` | how many n-gram candidates are re-ranked (default 20) |
| `blend_alpha` | weight of the model in the final score, 0..1 |
| `license` | licence of the weights |

`ModelManifest::validate(dir)` checks the fields, that both files exist, and their sizes. Hash verification is done when the model is loaded.

## User dictionary (opt-in, on device, not implemented)

`khcore::userdict::UserDictionary` is the storage interface for a dictionary that learns words the user accepts.
It must stay opt-in, never leave the device, never log typed text, and be easy to wipe. The first implementation
will be a small file in the user's app-data folder; the default is `NullUserDictionary`, which learns nothing.
