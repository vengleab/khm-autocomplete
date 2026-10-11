#!/usr/bin/env python3
"""
Build the frequency data file used by the Khmer autocomplete demo.

Three input modes:

  # 1) Pre-segmented text, one sentence per line, words separated by spaces
  #    (this is how seed_corpus.txt is written; no segmenter needed)
  python build_data.py --segmented seed_corpus.txt -o data.json --min-count 1

  # 2) Plain Khmer text (no spaces between words); segmented with khmer-nltk
  python build_data.py --text my_corpus.txt -o data.json

  # 3) A Khmer Wikipedia dump (download kmwiki-latest-pages-articles.xml.bz2 from
  #    https://dumps.wikimedia.org/kmwiki/latest/ on your own machine)
  python build_data.py --wiki kmwiki-latest-pages-articles.xml.bz2 -o data.json

Requires for modes 2 and 3:  pip install khmer-nltk
Then load data.json in the demo page with the "Load data file" button.
"""
import argparse, bz2, json, re, sys, unicodedata
from collections import Counter, defaultdict

# ---------------------------------------------------------------- normalization
# Keep this logic identical to normalize() in the demo page (index.html).
CLUSTER = re.compile(
    r"([ក-ឳ])((?:្[ក-អ]|[឴-៓៝])*)"
)
SHIFTERS = {"៉", "៊"}


def _mark_groups(marks):
    """Split a cluster's marks into units: (coeng + consonant) pairs and single marks."""
    out, i = [], 0
    while i < len(marks):
        if marks[i] == "្" and i + 1 < len(marks):
            out.append(marks[i : i + 2])
            i += 2
        else:
            out.append(marks[i])
            i += 1
    return out


def _rank(unit):
    c = unit[0]
    if c == "្":
        return 1                      # subscript consonants first
    if c in SHIFTERS:
        return 2                      # register shifters (muusikatoan, triisap)
    if "឴" <= c <= "ៅ":
        return 3                      # vowel signs
    return 4                          # other signs (nikahit, bantoc, ...)


def _fix_cluster(m):
    units = _mark_groups(m.group(2))
    units.sort(key=_rank)             # stable sort
    return m.group(1) + "".join(units)


def normalize(s: str) -> str:
    s = unicodedata.normalize("NFC", s)
    s = re.sub("[​‌‍⁠﻿]", "", s)   # invisible separators
    return CLUSTER.sub(_fix_cluster, s)


# ------------------------------------------------------------------- tokenizing
WORD_OK = re.compile(r"^[ក-៓៝០-៩]+$")
KHMER_RUN = re.compile(r"[ក-៿​ ]{2,}")
_segmenter = None


def segment(text: str):
    global _segmenter
    if _segmenter is None:
        try:
            from khmernltk import word_tokenize
        except ImportError:
            sys.exit("Install the segmenter first:  pip install khmer-nltk")
        _segmenter = word_tokenize
    return _segmenter(text, return_tokens=True)


def clean_token(t):
    t = normalize(t.strip())
    return t if t and WORD_OK.match(t) and t != "ៗ" else None


def sentences_from_segmented(path):
    with open(path, encoding="utf-8") as f:
        for line in f:
            toks = [clean_token(t) for t in line.split()]
            toks = [t for t in toks if t]
            if toks:
                yield toks


def sentences_from_text(lines):
    for line in lines:
        for chunk in re.split(r"[។៕៖\n!?]", line):
            chunk = normalize(chunk.replace("​", " "))
            if len(re.findall(r"[ក-ឳ]", chunk)) < 3:
                continue
            for run in KHMER_RUN.findall(chunk):
                toks = [clean_token(t) for t in segment(run.replace(" ", ""))]
                toks = [t for t in toks if t]
                if toks:
                    yield toks


WIKI_JUNK = [
    (re.compile(r"\{\{[^{}]*\}\}"), " "),
    (re.compile(r"\[\[(?:File|Image|ឯកសារ|រូបភាព|Category|ចំណាត់ថ្នាក់ក្រុម)[^\]]*\]\]", re.I), " "),
    (re.compile(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]"), r"\1"),
    (re.compile(r"<ref[^>]*>.*?</ref>|<[^>]+>", re.S), " "),
    (re.compile(r"https?://\S+"), " "),
    (re.compile(r"'{2,}"), ""),
]


def wiki_lines(path):
    import xml.etree.ElementTree as ET
    opener = bz2.open if path.endswith(".bz2") else open
    with opener(path, "rb") as f:
        for _, el in ET.iterparse(f):
            if el.tag.endswith("}text") or el.tag == "text":
                txt = el.text or ""
                for pat, rep in WIKI_JUNK:
                    txt = pat.sub(rep, txt)
                for line in txt.split("\n"):
                    if not line.startswith(("=", "|", "!", "{", "*", "#", ":")):
                        yield line
                el.clear()


# ------------------------------------------------------------------------ build
def build(sentences, min_count, bigram_min, bigram_top):
    uni, bi = Counter(), defaultdict(Counter)
    n_sent = n_tok = 0
    for toks in sentences:
        n_sent += 1
        n_tok += len(toks)
        uni.update(toks)
        prev = "^"                                 # sentence start marker
        for t in toks:
            bi[prev][t] += 1
            prev = t
    vocab = [w for w, c in uni.most_common() if c >= min_count]
    index = {w: i for i, w in enumerate(vocab)}
    out_bi = {}
    for prev, nxt in bi.items():
        if prev != "^" and prev not in index:
            continue
        pairs = [(index[w], c) for w, c in nxt.most_common()
                 if w in index and c >= bigram_min][:bigram_top]
        if pairs:
            key = "^" if prev == "^" else str(index[prev])
            out_bi[key] = [x for p in pairs for x in p]   # flat [j, count, j, count...]
    return {
        "meta": {"sentences": n_sent, "tokens": n_tok, "vocab": len(vocab)},
        "words": vocab,
        "freq": [uni[w] for w in vocab],
        "bi": out_bi,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--segmented", help="pre-segmented text file (space-separated words)")
    g.add_argument("--text", help="plain Khmer text file (will be segmented)")
    g.add_argument("--wiki", help="Khmer Wikipedia XML dump (.xml or .xml.bz2)")
    ap.add_argument("-o", "--out", default="data.json")
    ap.add_argument("--source", default=None, help="label shown in the demo")
    ap.add_argument("--min-count", type=int, default=3, help="drop words rarer than this")
    ap.add_argument("--bigram-min", type=int, default=2)
    ap.add_argument("--bigram-top", type=int, default=20, help="max next words kept per word")
    a = ap.parse_args()

    if a.segmented:
        sents, label = sentences_from_segmented(a.segmented), "seed corpus"
    elif a.text:
        sents, label = sentences_from_text(open(a.text, encoding="utf-8")), "custom text"
    else:
        sents, label = sentences_from_text(wiki_lines(a.wiki)), "Khmer Wikipedia"

    data = build(sents, a.min_count, a.bigram_min, a.bigram_top)
    data["meta"]["source"] = a.source or label
    with open(a.out, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, separators=(",", ":"))
    m = data["meta"]
    print(f"wrote {a.out}: {m['vocab']} words, {m['tokens']} tokens, "
          f"{len(data['bi'])} bigram rows ({m['source']})")


if __name__ == "__main__":
    main()
