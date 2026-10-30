#!/usr/bin/env python3
"""The local database behind the admin pages: the consonant / vowel mapping, and the Khmer word dictionary
(vocabulary, Khmerlish spellings = the lexicon, English glosses = the translation).

SQLite, standard library only. The JSON files in data/ stay the interchange format the engine reads; this module
seeds the database from them and exports back to them:

  python3 admin/khdb.py seed      # create data/khmer.db from data/*.json (add --force to replace an existing one)
  python3 admin/khdb.py export    # write data/mapping.json, lexicon.json, english.json, data.json from the database
  python3 admin/khdb.py stats

The engine's rules for a valid mapping live in core/src/tables.rs (`Tables::from_json`); `validate_mapping` here
applies the same rules, so a mapping saved by the admin pages always loads.
"""
import argparse
import importlib.util
import json
import os
import re
import sqlite3
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
DATA = os.path.join(ROOT, "data")
DEFAULT_MAPPING = os.path.join(HERE, "default_mapping.json")
DEFAULT_DB = os.path.join(DATA, "khmer.db")

SCHEMA = """
CREATE TABLE IF NOT EXISTS consonants (
  pos    INTEGER PRIMARY KEY,
  letter TEXT NOT NULL UNIQUE,
  series TEXT NOT NULL CHECK (series IN ('A', 'O')),
  onset  TEXT NOT NULL,
  final  TEXT NOT NULL,
  to_a   INTEGER NOT NULL DEFAULT 0,
  to_o   INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS vowels (
  pos  INTEGER PRIMARY KEY,
  sign TEXT NOT NULL UNIQUE,
  a    TEXT NOT NULL,
  o    TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS onset_extra (
  pos     INTEGER PRIMARY KEY,
  latin   TEXT NOT NULL UNIQUE,
  letters TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS finals (
  pos     INTEGER PRIMARY KEY,
  latin   TEXT NOT NULL UNIQUE,
  letters TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS independent (
  pos    INTEGER PRIMARY KEY,
  letter TEXT NOT NULL UNIQUE,
  latin  TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS mapping_log (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  saved_at TEXT NOT NULL DEFAULT (datetime('now')),
  mapping  TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS words (
  idx   INTEGER PRIMARY KEY,
  word  TEXT NOT NULL UNIQUE,
  freq  INTEGER NOT NULL CHECK (freq >= 1),
  added INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS lexicon (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  word      TEXT NOT NULL UNIQUE,
  spellings TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS glosses (
  id      INTEGER PRIMARY KEY AUTOINCREMENT,
  word    TEXT NOT NULL UNIQUE,
  glosses TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"""

LOWER = re.compile(r"^[a-z]*$")
KHMER = re.compile(r"[ក-៿]")
SPELLING = re.compile(r"^[A-Za-z'\-]+$")
# the signs the Khmer -> Latin direction can meet: each needs a row with a reading in both series
VOWSIGNS = list("ាិីឹឺុូួើឿៀេែៃោៅ")
REQUIRED_SIGNS = [""] + VOWSIGNS + ["ំ", "ះ"]
MAPPING_LOG_KEEP = 30


class ValidationError(ValueError):
    """The input is not acceptable; `.errors` lists every problem."""

    def __init__(self, errors):
        self.errors = [errors] if isinstance(errors, str) else list(errors)
        super().__init__("; ".join(self.errors))


# ------------------------------------------------------------------------------------------------ connection
def connect(path=DEFAULT_DB):
    conn = sqlite3.connect(path)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    conn.executescript(SCHEMA)
    return conn


def _normalizer():
    """`normalize()` of data/build_data.py: the same text rules as the engine (NFC, invisible characters, mark order)."""
    spec = importlib.util.spec_from_file_location("build_data", os.path.join(DATA, "build_data.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.normalize


_normalize = None


def normalize(text):
    global _normalize
    if _normalize is None:
        _normalize = _normalizer()
    return _normalize(text)


def _read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


# ------------------------------------------------------------------------------------------------ mapping
def validate_mapping(m):
    """Every problem with a mapping (the format of data/mapping.json), as a list of strings; empty when it is valid."""
    errs = []
    if not isinstance(m, dict):
        return ["the mapping must be an object"]
    cons = m.get("consonants")
    if not isinstance(cons, list) or not cons:
        return ["consonants: a non-empty list is required"]
    letters = set()
    for i, c in enumerate(cons):
        where = "consonant #%d" % (i + 1)
        if not isinstance(c, dict):
            errs.append(where + ": not an object")
            continue
        l = c.get("letter")
        if not isinstance(l, str) or len(l) != 1:
            errs.append("%s: letter must be exactly one character" % where)
            continue
        where = "consonant " + l
        if l in letters:
            errs.append("%s: listed twice" % where)
        letters.add(l)
        if c.get("series") not in ("A", "O"):
            errs.append("%s: series must be A or O" % where)
        for key, label in (("onset", "onset"), ("final", "final")):
            v = c.get(key, "")
            if not isinstance(v, str) or not LOWER.match(v):
                errs.append("%s: %s must be lowercase a-z (or empty)" % (where, label))
    vows = m.get("vowels")
    if not isinstance(vows, list):
        errs.append("vowels: a list is required")
        vows = []
    signs = {}
    for i, v in enumerate(vows):
        if not isinstance(v, dict) or not isinstance(v.get("sign"), str):
            errs.append("vowel #%d: sign is required" % (i + 1))
            continue
        s = v["sign"]
        name = "vowel %s" % (s or "(inherent)")
        if s in signs:
            errs.append("%s: listed twice" % name)
        signs[s] = v
        for ser in ("A", "O"):
            lst = v.get(ser)
            if not isinstance(lst, list) or any(not isinstance(x, str) or not x or not LOWER.match(x) for x in lst):
                errs.append("%s: series %s readings must be non-empty lowercase a-z words" % (name, ser))
    for s in REQUIRED_SIGNS:
        v = signs.get(s)
        if v is None:
            errs.append("vowel %s is missing (the engine needs it)" % (s or "(inherent)"))
        elif not (isinstance(v.get("A"), list) and v["A"] and isinstance(v.get("O"), list) and v["O"]):
            errs.append("vowel %s needs at least one reading in each series" % (s or "(inherent)"))
    for key in ("onsetExtra", "finals"):
        seen = set()
        lst = m.get(key, [])
        if not isinstance(lst, list):
            errs.append("%s: a list is required" % key)
            continue
        for i, e in enumerate(lst):
            if not isinstance(e, dict):
                errs.append("%s #%d: not an object" % (key, i + 1))
                continue
            lat, ls = e.get("latin"), e.get("letters")
            if not isinstance(lat, str) or not lat or not re.match(r"^[a-z]+$", lat):
                errs.append("%s #%d: latin must be lowercase a-z and not empty" % (key, i + 1))
                continue
            if lat in seen:
                errs.append("%s %s: listed twice" % (key, lat))
            seen.add(lat)
            if not isinstance(ls, str) or not ls or any(ch not in letters for ch in ls):
                errs.append("%s %s: letters must be a non-empty string of listed consonants" % (key, lat))
    seen = set()
    for i, e in enumerate(m.get("independent", [])):
        if not isinstance(e, dict) or not isinstance(e.get("letter"), str) or len(e["letter"]) != 1:
            errs.append("independent vowel #%d: letter must be exactly one character" % (i + 1))
            continue
        if e["letter"] in seen:
            errs.append("independent vowel %s: listed twice" % e["letter"])
        seen.add(e["letter"])
        if not isinstance(e.get("latin"), str) or not LOWER.match(e["latin"]):
            errs.append("independent vowel %s: latin must be lowercase a-z" % e["letter"])
    return errs


def format_mapping(m):
    """data/mapping.json as text: one entry per line inside each list, easy to read and to diff."""

    def lst(items):
        return "[\n  " + ",\n  ".join(json.dumps(i, ensure_ascii=False) for i in items) + "\n ]"

    return (
        '{\n "version": 1,\n "consonants": %s,\n "onsetExtra": %s,\n "vowels": %s,\n "finals": %s,\n "independent": %s\n}\n'
        % (lst(m["consonants"]), lst(m.get("onsetExtra", [])), lst(m["vowels"]), lst(m.get("finals", [])), lst(m.get("independent", [])))
    )


def get_mapping(conn):
    q = lambda sql: conn.execute(sql).fetchall()
    return {
        "version": 1,
        "consonants": [
            {"letter": r["letter"], "series": r["series"], "onset": r["onset"], "final": r["final"], "toA": bool(r["to_a"]), "toO": bool(r["to_o"])}
            for r in q("SELECT * FROM consonants ORDER BY pos")
        ],
        "onsetExtra": [{"latin": r["latin"], "letters": r["letters"]} for r in q("SELECT * FROM onset_extra ORDER BY pos")],
        "vowels": [{"sign": r["sign"], "A": r["a"].split(), "O": r["o"].split()} for r in q("SELECT * FROM vowels ORDER BY pos")],
        "finals": [{"latin": r["latin"], "letters": r["letters"]} for r in q("SELECT * FROM finals ORDER BY pos")],
        "independent": [{"letter": r["letter"], "latin": r["latin"]} for r in q("SELECT * FROM independent ORDER BY pos")],
    }


def _write_mapping_rows(conn, m):
    for t in ("consonants", "vowels", "onset_extra", "finals", "independent"):
        conn.execute("DELETE FROM " + t)
    for i, c in enumerate(m["consonants"]):
        conn.execute(
            "INSERT INTO consonants VALUES (?,?,?,?,?,?,?)",
            (i, c["letter"], c["series"], c.get("onset", ""), c.get("final", ""), int(bool(c.get("toA"))), int(bool(c.get("toO")))),
        )
    for i, v in enumerate(m["vowels"]):
        conn.execute("INSERT INTO vowels VALUES (?,?,?,?)", (i, v["sign"], " ".join(v["A"]), " ".join(v["O"])))
    for i, e in enumerate(m.get("onsetExtra", [])):
        conn.execute("INSERT INTO onset_extra VALUES (?,?,?)", (i, e["latin"], e["letters"]))
    for i, e in enumerate(m.get("finals", [])):
        conn.execute("INSERT INTO finals VALUES (?,?,?)", (i, e["latin"], e["letters"]))
    for i, e in enumerate(m.get("independent", [])):
        conn.execute("INSERT INTO independent VALUES (?,?,?)", (i, e["letter"], e["latin"]))


def set_mapping(conn, m, log=True):
    """Replace the whole mapping. Raises ValidationError (nothing is written) when it is not valid."""
    errs = validate_mapping(m)
    if errs:
        raise ValidationError(errs)
    with conn:
        if log and conn.execute("SELECT COUNT(*) FROM consonants").fetchone()[0]:
            conn.execute("INSERT INTO mapping_log (mapping) VALUES (?)", (json.dumps(get_mapping(conn), ensure_ascii=False),))
            conn.execute("DELETE FROM mapping_log WHERE id NOT IN (SELECT id FROM mapping_log ORDER BY id DESC LIMIT ?)", (MAPPING_LOG_KEEP,))
        _write_mapping_rows(conn, m)


def default_mapping():
    return _read_json(DEFAULT_MAPPING)


def mapping_history(conn):
    return [{"id": r["id"], "savedAt": r["saved_at"]} for r in conn.execute("SELECT id, saved_at FROM mapping_log ORDER BY id DESC")]


def mapping_version(conn, version_id):
    r = conn.execute("SELECT mapping FROM mapping_log WHERE id = ?", (version_id,)).fetchone()
    return json.loads(r["mapping"]) if r else None


# ------------------------------------------------------------------------------------------------ dictionary
def _clean_spellings(text):
    toks = (text or "").split()
    bad = [t for t in toks if not SPELLING.match(t)]
    if bad:
        raise ValidationError("spellings may use letters, ' and - only: %s" % ", ".join(bad))
    seen, out = set(), []
    for t in toks:
        if t.lower() not in seen:
            seen.add(t.lower())
            out.append(t)
    return " ".join(out)


def _clean_glosses(text):
    out, seen = [], set()
    for g in (text or "").split(","):
        g = " ".join(g.split())
        if g and g.lower() not in seen:
            seen.add(g.lower())
            out.append(g)
    return ", ".join(out)


def _clean_word(word):
    w = normalize((word or "").strip())
    if not w or re.search(r"\s", w):
        raise ValidationError("a word is one piece of Khmer text without spaces")
    if not KHMER.search(w):
        raise ValidationError("a word needs Khmer letters")
    return w


def _row(r):
    return {
        "word": r["word"], "idx": r["idx"], "freq": r["freq"], "added": bool(r["added"]),
        "spellings": r["spellings"] or "", "glosses": r["glosses"] or "",
    }


_SELECT = (
    "SELECT w.idx, w.word, w.freq, w.added, l.spellings, g.glosses FROM words w "
    "LEFT JOIN lexicon l ON l.word = w.word LEFT JOIN glosses g ON g.word = w.word"
)


def list_words(conn, q="", flt="all", offset=0, limit=50):
    where, args = [], []
    if q:
        like = "%" + q.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "%"
        where.append("(w.word LIKE ? ESCAPE '\\' OR l.spellings LIKE ? ESCAPE '\\' OR g.glosses LIKE ? ESCAPE '\\')")
        args += [like, like, like]
    if flt == "no_spelling":
        where.append("l.spellings IS NULL")
    elif flt == "no_gloss":
        where.append("g.glosses IS NULL")
    elif flt == "added":
        where.append("w.added = 1")
    clause = (" WHERE " + " AND ".join(where)) if where else ""
    total = conn.execute("SELECT COUNT(*) FROM words w LEFT JOIN lexicon l ON l.word = w.word LEFT JOIN glosses g ON g.word = w.word" + clause, args).fetchone()[0]
    rows = conn.execute(_SELECT + clause + " ORDER BY w.idx LIMIT ? OFFSET ?", args + [max(1, min(int(limit), 500)), max(0, int(offset))]).fetchall()
    return {"total": total, "rows": [_row(r) for r in rows]}


def get_word(conn, word):
    r = conn.execute(_SELECT + " WHERE w.word = ?", (word,)).fetchone()
    return _row(r) if r else None


def save_word(conn, word, freq=None, spellings=None, glosses=None, create=False):
    """Create (`create=True`, appended to the vocabulary) or update a word. `None` leaves a field as it is; an empty
    string clears spellings / glosses. Returns the saved row."""
    w = _clean_word(word)
    with conn:
        cur = conn.execute("SELECT idx FROM words WHERE word = ?", (w,)).fetchone()
        if create:
            if cur:
                raise ValidationError("%s is already in the dictionary" % w)
            idx = conn.execute("SELECT COALESCE(MAX(idx), -1) + 1 FROM words").fetchone()[0]
            f = 1 if freq is None else freq
            if not isinstance(f, int) or f < 1:
                raise ValidationError("frequency must be a whole number of at least 1")
            conn.execute("INSERT INTO words (idx, word, freq, added) VALUES (?,?,?,1)", (idx, w, f))
        else:
            if not cur:
                raise ValidationError("%s is not in the dictionary" % w)
            if freq is not None:
                if not isinstance(freq, int) or freq < 1:
                    raise ValidationError("frequency must be a whole number of at least 1")
                conn.execute("UPDATE words SET freq = ? WHERE word = ?", (freq, w))
        if spellings is not None:
            sp = _clean_spellings(spellings)
            if sp:
                conn.execute("INSERT INTO lexicon (word, spellings) VALUES (?,?) ON CONFLICT(word) DO UPDATE SET spellings = excluded.spellings", (w, sp))
            else:
                conn.execute("DELETE FROM lexicon WHERE word = ?", (w,))
        if glosses is not None:
            gl = _clean_glosses(glosses)
            if gl:
                conn.execute("INSERT INTO glosses (word, glosses) VALUES (?,?) ON CONFLICT(word) DO UPDATE SET glosses = excluded.glosses", (w, gl))
            else:
                conn.execute("DELETE FROM glosses WHERE word = ?", (w,))
    return get_word(conn, w)


def delete_word(conn, word):
    """A word added through the pages is removed completely. A word of the original vocabulary stays (the bigram data
    refers to it by position) but loses its spellings and glosses. Returns "removed", "cleared" or None."""
    w = normalize(word)
    with conn:
        r = conn.execute("SELECT added FROM words WHERE word = ?", (w,)).fetchone()
        if not r:
            return None
        conn.execute("DELETE FROM lexicon WHERE word = ?", (w,))
        conn.execute("DELETE FROM glosses WHERE word = ?", (w,))
        if r["added"]:
            conn.execute("DELETE FROM words WHERE word = ?", (w,))
            return "removed"
        return "cleared"


def stats(conn):
    n = lambda sql: conn.execute(sql).fetchone()[0]
    return {
        "words": n("SELECT COUNT(*) FROM words"), "added": n("SELECT COUNT(*) FROM words WHERE added = 1"),
        "withSpellings": n("SELECT COUNT(*) FROM lexicon"), "withGlosses": n("SELECT COUNT(*) FROM glosses"),
        "consonants": n("SELECT COUNT(*) FROM consonants"), "vowels": n("SELECT COUNT(*) FROM vowels"),
        "mappingVersions": n("SELECT COUNT(*) FROM mapping_log"),
    }


# ------------------------------------------------------------------------------------------------ seed / export
def is_seeded(conn):
    return conn.execute("SELECT COUNT(*) FROM words").fetchone()[0] > 0


def seed(conn, data_dir=DATA, force=False):
    """Fill the database from data/data.json, lexicon.json, english.json and mapping.json (the default mapping when
    the last one is missing). With `force` an existing database content is replaced."""
    if is_seeded(conn) and not force:
        raise ValidationError("the database already has content (use force to replace it)")
    data = _read_json(os.path.join(data_dir, "data.json"))
    lex = _read_json(os.path.join(data_dir, "lexicon.json"))
    eng_path = os.path.join(data_dir, "english.json")
    eng = _read_json(eng_path) if os.path.exists(eng_path) else {}
    map_path = os.path.join(data_dir, "mapping.json")
    mapping = _read_json(map_path) if os.path.exists(map_path) else default_mapping()
    if len(data["words"]) != len(data["freq"]):
        raise ValidationError("data.json: words and freq differ in length")
    with conn:
        for t in ("words", "lexicon", "glosses", "kv", "mapping_log"):
            conn.execute("DELETE FROM " + t)
        conn.executemany("INSERT INTO words (idx, word, freq, added) VALUES (?,?,?,0)", [(i, w, f) for i, (w, f) in enumerate(zip(data["words"], data["freq"]))])
        conn.executemany("INSERT OR REPLACE INTO lexicon (word, spellings) VALUES (?,?)", list(lex.items()))
        conn.executemany("INSERT OR REPLACE INTO glosses (word, glosses) VALUES (?,?)", list(eng.items()))
        meta = data.get("meta", {})
        conn.execute("INSERT INTO kv VALUES ('meta', ?)", (json.dumps(meta, ensure_ascii=False),))
        conn.execute("INSERT INTO kv VALUES ('bi', ?)", (json.dumps(data.get("bi", {}), separators=(",", ":")),))
        conn.execute("INSERT INTO kv VALUES ('orig_freq_sum', ?)", (str(sum(data["freq"])),))
    errs = validate_mapping(mapping)
    if errs:
        raise ValidationError(["mapping.json: " + e for e in errs])
    set_mapping(conn, mapping, log=False)
    return stats(conn)


def _atomic_write(path, text):
    d = os.path.dirname(path)
    fd, tmp = tempfile.mkstemp(dir=d, prefix=".tmp-", suffix=".json")
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        os.replace(tmp, path)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise


def export_texts(conn):
    """The four interchange files as text: {file name: text}."""
    words = conn.execute("SELECT word, freq, added FROM words ORDER BY idx").fetchall()
    kv = {r["key"]: r["value"] for r in conn.execute("SELECT * FROM kv")}
    meta = json.loads(kv.get("meta", "{}"))
    bi = json.loads(kv.get("bi", "{}"))
    orig_sum = int(kv.get("orig_freq_sum", "0"))
    total = sum(r["freq"] for r in words)
    meta["vocab"] = len(words)
    if "tokens" in meta:
        meta["tokens"] = max(0, meta["tokens"] + total - orig_sum)
    data = {"meta": meta, "words": [r["word"] for r in words], "freq": [r["freq"] for r in words], "bi": bi}
    lex = conn.execute("SELECT word, spellings FROM lexicon ORDER BY id").fetchall()
    eng = conn.execute("SELECT word, glosses FROM glosses ORDER BY id").fetchall()
    lex_text = "{\n" + ",\n".join("%s: %s" % (json.dumps(r["word"], ensure_ascii=False), json.dumps(r["spellings"], ensure_ascii=False)) for r in lex) + "\n}"
    return {
        "data.json": json.dumps(data, ensure_ascii=False, separators=(",", ":")),
        "lexicon.json": lex_text,
        "english.json": json.dumps({r["word"]: r["glosses"] for r in eng}, ensure_ascii=False, indent=2) + "\n",
        "mapping.json": format_mapping(get_mapping(conn)),
    }


def export_files(conn, data_dir=DATA, only=None):
    """Write the interchange files (all four, or the names in `only`). Returns {name: bytes written}."""
    texts = export_texts(conn)
    done = {}
    for name, text in texts.items():
        if only and name not in only:
            continue
        _atomic_write(os.path.join(data_dir, name), text)
        done[name] = len(text.encode("utf-8"))
    return done


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["seed", "export", "stats"])
    ap.add_argument("--db", default=DEFAULT_DB)
    ap.add_argument("--data", default=DATA, help="folder of the JSON files (default: data/)")
    ap.add_argument("--force", action="store_true", help="seed: replace existing content")
    a = ap.parse_args(argv)
    conn = connect(a.db)
    try:
        if a.command == "seed":
            print(json.dumps(seed(conn, a.data, a.force), indent=1))
        elif a.command == "export":
            print(json.dumps(export_files(conn, a.data), indent=1))
        else:
            print(json.dumps(stats(conn), indent=1))
    except ValidationError as e:
        sys.exit("error: " + str(e))
    return 0


if __name__ == "__main__":
    main()
