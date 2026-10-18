# Khmer autocomplete demo

Four input modes in one page. **Auto** (the default) is the one the keyboards use: a single Khmer state that works out whether you
typed Khmerlish, an English word, keys of the Khmer (NiDA) layout, or Khmer script. The other three are kept to compare:
- Auto: type `knhom chong`, `water` or `xjJuM` (the Khmer layout keys of ខ្ញុំ) and press Space; nothing is remapped, the page reads the run.
- Khmer script: word completion, next-word prediction, typo fixes, on-screen Khmer keyboard.
- Latin input (Khmerlish): type Khmer in Latin letters ("knhom chong nham bay"), press Space to convert each word to Khmer script.
- English: type an English word ("water", "eating"), press Space to turn it into Khmer. Uses data/english.json, a small hand-written glossary.

Admin pages (a local SQLite database and two web pages to edit the consonant / vowel mapping and the word dictionary):
`python3 admin/server.py`, then open http://127.0.0.1:8765/ (see admin/README.md).

Files
- web/index.html    standalone demo (open in a browser; needs internet only for the Khmer web font)
- data/data.json    frequency data built from seed_corpus.txt (277 words)
- data/lexicon.json hand-written Latin spellings for each seed word (several variants per word)
- data/seed_corpus.txt small hand-written, pre-segmented Khmer corpus
- data/build_data.py builds data.json from a Khmer Wikipedia dump, plain text, or pre-segmented text
- web/template.html + web/build_demo.py   embed data.json and lexicon.json into the page -> web/index.html
- core/             Rust port of the engine (khcore); see CLAUDE.md

The series rule (from the transliteration charts)
- Every consonant is first series (A) or second series (O). The same vowel sign sounds different after each:
  ក+ា = ka, គ+ា = kea. The tables are in web/template.html as CONS and VOW and drive both directions.
- Khmer to Latin: romanize() picks the reading from the consonant's series. ៉ makes a second-series consonant read
  like the first series, ៊ makes a first-series consonant read like the second. Paired signs (ុំ, ាំ, ះ) use their own readings.
- Latin to Khmer: compose() works the charts backwards for syllables that are not in the word list. The vowel sound
  decides the series, which decides the letter (k + "ea" can only be គ). Results are marked "by rule" and are guesses,
  because Khmer spelling is not fully phonetic (ត/ដ/ទ, subscripts, silent letters).
- The page has a "Series rule" panel that prints both charts from the same data.

How Latin input works
1. Each Latin spelling is reduced to a sound key (kh/k, ph/p, ou/u, ea/ia/ie, knyom/knhom ...).
2. Words match by exact key, key prefix, or a one or two letter difference.
3. Matches are ranked by match quality, then by the next-word model, so context picks between homophones.
4. Words without a lexicon entry get a rule-based spelling generated from the Khmer script. This is approximate
   (inherent vowels and syllable finals are guessed), so for real use grow lexicon.json with common spellings.

Build real data (on your machine)
1. pip install khmer-nltk
2. Download https://dumps.wikimedia.org/kmwiki/latest/kmwiki-latest-pages-articles.xml.bz2
3. python data/build_data.py --wiki kmwiki-latest-pages-articles.xml.bz2 -o data.json
4. Either click "Load data file" in the demo and pick data.json,
   or run: python web/build_demo.py data/data.json   (embeds it into index.html)

Tuning: --min-count (default 3) drops rare words, --bigram-top (default 20) keeps the top next-words per word.
