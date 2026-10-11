# Khmer autocomplete keyboard

A lightweight Khmer autocomplete input method for macOS and Windows. Two modes: Khmer script (complete the word,
predict the next, fix typos) and Latin / Khmerlish ("knhom chong" -> Khmer; Space converts the word, Tab takes the
first suggestion). A browser prototype (`web/template.html`) is the **reference implementation**; the portable Rust
core (`core/`) is a port of it. When the two disagree, the JS is right, and the difference is reported, not fixed silently.

## Layout
```
core/        Rust crate `khcore`: all language logic. No platform code, no network, no logging of typed text.
  src/       cluster, norm, tables, romanize, compose, romkey, fdlibm (ln), model (data + compiled format),
             suggest (segmentation + both suggestion paths), scorer (Scorer trait), engine (public API), key (Space/Enter/Tab)
  src/bin/khbuild.rs   data.json + lexicon.json -> khdata.bin (compiled, fast-loading)
  tests/     golden parity, series charts, flows, perf     examples/perf.rs  memory check
capi/ wasm/  (Phase 1)   mac/ windows/  (Phases 2, 3)   models/  (Phase 4)
data/        seed_corpus.txt, lexicon.json, data.json, build_data.py   (data tools, unchanged)
web/         template.html (the demo = JS reference), build_demo.py -> index.html
tests/       check.mjs (Playwright, web demo)    tests/golden/  vectors generated from the JS reference
```

## Commands
- Rust tests (fast, debug):      `cargo test --workspace`
- Rust tests incl. perf (release): `cargo test --release --workspace -- --nocapture`
- Regenerate golden vectors:     `node tests/golden/gen.mjs`   (deterministic, ~4 min; output must not change unless the JS changed)
- Compile data:                  `cargo run --release -p khcore --bin khbuild -- data/data.json data/lexicon.json khdata.bin`
- Memory check (fresh process):  `cargo run --release -p khcore --example perf -- gen 100000 /tmp/k.bin` then `... -- run /tmp/k.bin`
- Web demo build + test:         `npm install && npm test`   (needs Playwright's Chromium; here: `CHROMIUM_PATH=/opt/pw-browsers/chromium-1194/chrome-linux/chrome npm test`)
- Rebuild data:                  `python3 data/build_data.py --segmented data/seed_corpus.txt -o data/data.json --min-count 1 --bigram-min 1 --source "Seed corpus (demo)"`
- Real data:                     `pip install khmer-nltk` then `python3 data/build_data.py --wiki kmwiki-latest-pages-articles.xml.bz2 -o data/data.json` (the Wikipedia download is the owner's job)

## Rules that matter
- **JS is the reference.** `tests/golden/gen.mjs` runs `web/template.html` unmodified (it evals the script up to the unique `UI` anchor
  comment) and records outputs; `core/tests/golden_*.rs` must reproduce them exactly. Change JS first, regenerate, then port.
- Normalization lives in three places that must stay identical: `normalize()` in `data/build_data.py`, in `web/template.html`, and `core/src/norm.rs`.
- The series rule is data: `CONS`, `VOW` in `template.html`, `core/src/tables.rs` in Rust (a golden test compares the two). Both directions
  (`romanize`, `compose`) read the tables. Change the tables, not the functions, when fixing a reading.
- `romKey()` reduces a Latin spelling to a sound key. Any change to it changes every match: rerun all tests.
- Names `CONS`, `VOW`, `SIGN` are taken by the series tables in the JS; the on-screen keyboard uses `KB_CONS`, `KB_VOW`, `KB_SIGN`.
- Do not splice the template by searching for comment text like `/* samples */`; use unique anchors.
- Khmer needs a Khmer font to judge rendering. Check visually in a real browser, not only in headless tests.
- Shells (Swift, C#, Rust TSF) are thin: keystrokes and text-before-caret in, candidates out, insert the chosen text. No language logic there.
  The Space/Enter/Tab rules live in `Engine::key_action`.
- No network calls, no logging of typed text, suggestions off in password / secure-input fields (shell responsibility).

## Decisions (Phase 0)
- `icu_segmenter` is **not used**: `clusters()` is a regex-defined unit (coeng + consonant), not UAX #29, and word segmentation is the specified DP.
  Revisit behind an optional feature for corpus tooling.
- The compiled file is read into memory (safe code only). An `mmap` feature would need one `unsafe`; not added yet (50 MB budget is met without it).
- `fst` for the word index (word -> id, prefix ranges), `unicode-normalization` for NFC/NFD, `serde`/`serde_json` for data.json. No `libm`:
  `core/src/fdlibm.rs` is a port of fdlibm's `log`, the algorithm behind V8's `Math.log`. `libm::log` and the system libm differ from V8 in the
  last bit on ~0.3% of inputs, which could flip a segmentation tie; the golden test checks 25k values bit for bit.
- Scores are f64 inside (like JS), `Candidate.prob` is f32 at the edge. Candidates are built only for the survivors of the top-8 cut.
- `Candidate.replace_len_utf16` is measured in the **raw** text the shell sent (normalization may drop/reorder characters), so a shell can
  use it directly as the replacement range.
- Compiled format `khdata.bin` v1 (`KHDATA01`, little endian): meta JSON, words, freq, bigram CSR, Latin sound index, display spellings, fst.
  Loading validates structure and the fst checksum, so a corrupted file is an `Err`, never a panic (fuzz-tested).

## JS behaviors the port reproduces on purpose (differences from the written spec)
1. `romKey` maps `ə ɛ ɨ ɔ ɑ` before lowercasing, so uppercase `Ə Ɛ Ɔ` are deleted, not mapped.
2. Empty partial: `next` and `common` candidates are sorted together by P (a common word can outrank a next word).
3. Segmentation caps a known word at `min(24, longest vocab word)` clusters.
4. Latin typo tier compares the token key with the entry key and with the entry key cut to the token key's length.
5. Runs are split with JS `\s` (includes U+FEFF and U+00A0, excludes U+0085).
6. `romanize`: the `"m"` branch for nikahit after vowels other than ា/ុ is dead code in JS, so e.g. `កើំ` reads "kaeu". Ported as is. Open question for the owner.
7. Tab accepts the top candidate in both modes.
8. The JS rewrites all text before the caret on accept; an IME cannot, so the engine only replaces the pending token / partial.
9. Latin mode with no pending token falls back to Khmer-script suggestions (JS `refresh()`).

## Known limits
- Built-in data is tiny (277 words). Real coverage needs the Wikipedia build.
- Rule-made spellings (`compose()`) are guesses: Khmer spelling is not fully phonetic.
- Lexicon spellings are the author's, not collected from real users. Grow them from real chat text.
- Typo scans cover only the 20,000 most frequent words (as in the JS).

## Status
- Phase 0 (core + golden tests): done, see commit log. Phases 1-4: not started.
