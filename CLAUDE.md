# Khmer autocomplete demo

Browser demo of Khmer autocomplete with two modes: Khmer script (word completion, next-word prediction,
typo fixes, on-screen keyboard) and Latin input / Khmerlish (type "knhom chong", press Space, get Khmer script).

## Layout
- `template.html`   the whole app (HTML, CSS, JS in one file). Contains `__DATA__` and `__LEX__` placeholders. Edit this file.
- `build_demo.py`   embeds `data.json` and `lexicon.json` into the template -> `index.html` (standalone) and `artifact.html` (fragment, no doctype). Never edit these two by hand.
- `build_data.py`   builds `data.json` (word list, frequencies, bigrams) from a Wikipedia dump, plain text, or pre-segmented text.
- `seed_corpus.txt` small hand-written, pre-segmented corpus used for the built-in data.
- `lexicon.json`    hand-written Latin spellings per Khmer word (several variants each).
- `tests/check.mjs` Playwright checks: series charts, Latin to Khmer rules, a typing flow, no page errors.

## Commands
- Rebuild data:  `python3 build_data.py --segmented seed_corpus.txt -o data.json --min-count 1 --bigram-min 1 --source "Seed corpus (demo)"`
- Rebuild page:  `python3 build_demo.py data.json`
- Test:          `npm install && npx playwright install chromium && npm test`
- Real data:     `pip install khmer-nltk` then `python3 build_data.py --wiki kmwiki-latest-pages-articles.xml.bz2 -o data.json`

## Rules that matter
- Normalization lives in two places that must stay identical: `normalize()` in `build_data.py` and in `template.html`.
- The series rule (first series A / second series O) is data in `template.html`: `CONS` and `VOW`. Both directions use it:
  `romanize()` (Khmer to Latin) and `compose()` (Latin to Khmer). Change the tables, not the functions, when fixing a reading.
- `romKey()` reduces a Latin spelling to a sound key. Any change to it changes every match, so rerun the tests.
- Names `CONS`, `VOW`, `SIGN` are taken by the series tables; the on-screen keyboard uses `KB_CONS`, `KB_VOW`, `KB_SIGN`.
- Khmer needs a Khmer font to judge rendering. Check visually in a real browser, not only in headless tests.
- Do not splice the template by searching for comment text like `/* samples */`; it appears in both CSS and JS. Use unique anchors.

## Known limits
- Built-in data is tiny (277 words). Real coverage needs the Wikipedia build.
- Rule-made spellings (`compose()`) are guesses: Khmer spelling is not fully phonetic.
- Lexicon spellings are the author's, not collected from real users. Grow them from real chat text.
