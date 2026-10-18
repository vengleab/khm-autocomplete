#!/usr/bin/env python3
"""Tests for the database layer and the local server: python3 -m unittest discover -s admin -v"""
import copy
import http.client
import json
import os
import shutil
import sys
import tempfile
import threading
import unittest
import urllib.parse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import khdb  # noqa: E402
import server  # noqa: E402


def sandbox():
    """A temp folder with a copy of the data files and an empty database path."""
    d = tempfile.mkdtemp(prefix="khadmin-")
    for n in ("data.json", "lexicon.json", "english.json", "mapping.json", "build_data.py"):
        shutil.copy(os.path.join(khdb.DATA, n), d)
    return d


class DbTests(unittest.TestCase):
    def setUp(self):
        self.dir = sandbox()
        self.conn = khdb.connect(os.path.join(self.dir, "t.db"))
        khdb.seed(self.conn, self.dir)

    def tearDown(self):
        self.conn.close()
        shutil.rmtree(self.dir)

    def test_seed_then_export_is_byte_identical(self):
        out = tempfile.mkdtemp(prefix="khadmin-out-")
        try:
            khdb.export_files(self.conn, out)
            for n in ("data.json", "lexicon.json", "english.json", "mapping.json"):
                with open(os.path.join(khdb.DATA, n), "rb") as a, open(os.path.join(out, n), "rb") as b:
                    self.assertEqual(a.read(), b.read(), n + " changed on a round trip")
        finally:
            shutil.rmtree(out)

    def test_seed_refuses_to_overwrite(self):
        with self.assertRaises(khdb.ValidationError):
            khdb.seed(self.conn, self.dir)
        khdb.seed(self.conn, self.dir, force=True)

    def test_default_mapping_file_is_valid_and_matches_the_seed(self):
        self.assertEqual(khdb.validate_mapping(khdb.default_mapping()), [])
        self.assertEqual(khdb.get_mapping(self.conn), khdb.default_mapping())

    def test_mapping_validation_catches_what_the_engine_rejects(self):
        base = khdb.default_mapping()

        def bad(f):
            m = copy.deepcopy(base)
            f(m)
            return khdb.validate_mapping(m)

        self.assertTrue(bad(lambda m: m["consonants"][0].update(series="X")))
        self.assertTrue(bad(lambda m: m["consonants"][0].update(letter="កខ")))
        self.assertTrue(bad(lambda m: m["consonants"][1].update(letter="ក")))
        self.assertTrue(bad(lambda m: m["consonants"][0].update(onset="K")))
        self.assertTrue(bad(lambda m: m["vowels"].pop(1)))                       # a required sign is missing
        self.assertTrue(bad(lambda m: m["vowels"][2].update(A=[])))
        self.assertTrue(bad(lambda m: m["vowels"][2].update(A=[""])))
        self.assertTrue(bad(lambda m: m["vowels"][2].update(O=["E"])))
        self.assertTrue(bad(lambda m: m["finals"][0].update(letters="z")))
        self.assertTrue(bad(lambda m: m["finals"][0].update(latin="")))
        self.assertTrue(bad(lambda m: m["finals"].append({"latin": "k", "letters": "ក"})))   # duplicate
        self.assertTrue(khdb.validate_mapping({}))
        self.assertTrue(khdb.validate_mapping("nope"))
        # extra vowel rows are fine
        self.assertEqual(bad(lambda m: m["vowels"].append({"sign": "ុះ", "A": ["os"], "O": ["ous"]})), [])

    def test_set_mapping_is_all_or_nothing_and_keeps_history(self):
        m = khdb.get_mapping(self.conn)
        m["finals"].append({"latin": "b", "letters": "ប"})
        khdb.set_mapping(self.conn, m)
        self.assertIn({"latin": "b", "letters": "ប"}, khdb.get_mapping(self.conn)["finals"])
        self.assertEqual(len(khdb.mapping_history(self.conn)), 1)
        broken = copy.deepcopy(m)
        broken["consonants"][0]["series"] = "Z"
        before = khdb.get_mapping(self.conn)
        with self.assertRaises(khdb.ValidationError):
            khdb.set_mapping(self.conn, broken)
        self.assertEqual(khdb.get_mapping(self.conn), before)
        # the old version can be fetched back
        old = khdb.mapping_version(self.conn, khdb.mapping_history(self.conn)[0]["id"])
        self.assertEqual(old, khdb.default_mapping())

    def test_word_crud(self):
        self.assertEqual(khdb.list_words(self.conn, flt="no_gloss")["total"], 4)
        row = khdb.save_word(self.conn, "លាប", freq=3, spellings="leab  LEAP leap", glosses="paint,  apply, paint", create=True)
        self.assertEqual((row["spellings"], row["glosses"], row["added"], row["freq"]), ("leab LEAP", "paint, apply", True, 3))
        self.assertEqual(row["idx"], 277)
        with self.assertRaises(khdb.ValidationError):
            khdb.save_word(self.conn, "លាប", create=True)                       # already there
        with self.assertRaises(khdb.ValidationError):
            khdb.save_word(self.conn, "hello", create=True)                     # no Khmer
        with self.assertRaises(khdb.ValidationError):
            khdb.save_word(self.conn, "ក ខ", create=True)                       # a space
        with self.assertRaises(khdb.ValidationError):
            khdb.save_word(self.conn, "លាប", spellings="bad_spelling!")
        with self.assertRaises(khdb.ValidationError):
            khdb.save_word(self.conn, "លាប", freq=0)
        row = khdb.save_word(self.conn, "លាប", glosses="")                      # clears the translation, keeps the rest
        self.assertEqual((row["glosses"], row["spellings"]), ("", "leab LEAP"))
        self.assertEqual(khdb.list_words(self.conn, q="leab")["total"], 1)
        self.assertEqual(khdb.list_words(self.conn, flt="added")["total"], 1)
        # exported: appended to the vocabulary, bigram data untouched, meta updated
        texts = khdb.export_texts(self.conn)
        data = json.loads(texts["data.json"])
        self.assertEqual((data["words"][-1], data["freq"][-1], data["meta"]["vocab"]), ("លាប", 3, 278))
        self.assertEqual(data["bi"], json.load(open(os.path.join(khdb.DATA, "data.json"), encoding="utf-8"))["bi"])
        self.assertIn('"លាប": "leab LEAP"', texts["lexicon.json"])
        self.assertEqual(khdb.delete_word(self.conn, "លាប"), "removed")
        self.assertIsNone(khdb.get_word(self.conn, "លាប"))
        # a word of the original vocabulary is only cleared
        self.assertEqual(khdb.delete_word(self.conn, "ខ្ញុំ"), "cleared")
        w = khdb.get_word(self.conn, "ខ្ញុំ")
        self.assertEqual((w["spellings"], w["glosses"]), ("", ""))
        self.assertIsNone(khdb.delete_word(self.conn, "មិនមាន"))

    def test_search_filters_and_paging(self):
        self.assertEqual(khdb.list_words(self.conn)["total"], 277)
        page = khdb.list_words(self.conn, limit=10, offset=270)
        self.assertEqual(len(page["rows"]), 7)
        self.assertEqual(khdb.list_words(self.conn, q="knhom")["rows"][0]["word"], "ខ្ញុំ")
        self.assertEqual(khdb.list_words(self.conn, q="water")["rows"][0]["glosses"].split(", ")[0], "water")
        self.assertEqual(khdb.list_words(self.conn, q="100%")["total"], 0)       # LIKE wildcards are escaped
        self.assertEqual(khdb.list_words(self.conn, q="'; DROP TABLE words; --")["total"], 0)
        self.assertEqual(khdb.stats(self.conn)["words"], 277)


class ServerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = sandbox()
        cls.srv = server.make_server("127.0.0.1", 0, os.path.join(cls.dir, "t.db"), cls.dir)
        cls.port = cls.srv.server_address[1]
        threading.Thread(target=cls.srv.serve_forever, daemon=True).start()

    @classmethod
    def tearDownClass(cls):
        cls.srv.shutdown()
        cls.srv.server_close()
        shutil.rmtree(cls.dir)

    def call(self, method, path, body=None, headers=None):
        c = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        h = {"Content-Type": "application/json"} if method != "GET" else {}
        h.update(headers or {})
        c.request(method, path, json.dumps(body) if body is not None else None, h)
        r = c.getresponse()
        raw = r.read()
        c.close()
        ctype = r.getheader("Content-Type", "")
        return r.status, (json.loads(raw) if "json" in ctype and "javascript" not in ctype else raw.decode("utf-8")), r

    def test_pages_and_engine_are_served(self):
        for p, needle in (("/", "Khmer admin"), ("/mapping.html", "Consonants"), ("/dictionary.html", "dictionary"), ("/admin.css", "--accent"), ("/admin.js", "async function api")):
            st, body, _ = self.call("GET", p)
            self.assertEqual(st, 200, p)
            self.assertIn(needle, body, p)
        st, js, r = self.call("GET", "/engine.js")
        self.assertEqual(st, 200)
        self.assertIn("window.KhEngine", js)
        self.assertNotIn("__DATA__", js)
        self.assertNotIn("function refresh()", js, "only the engine part of the template is served")
        self.assertEqual(self.call("GET", "/nope")[0], 404)

    def test_engine_js_runs_in_node_with_the_database_mapping(self):
        node = shutil.which("node")
        if not node:
            self.skipTest("node is not installed")
        import subprocess
        st, js, _ = self.call("GET", "/engine.js")
        prog = "global.window = {};\n" + js + "\nconst K = window.KhEngine; const e = new K.Engine(K.SEED);\nconsole.log(JSON.stringify([K.romanize('ខ្ញុំ'), e.romSuggest('knhom', {prev:'^'})[0].word, K.compose('kea', e.prior)[0].word]));"
        out = subprocess.run([node, "-e", prog], capture_output=True, text=True, timeout=60)
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertEqual(json.loads(out.stdout), ["khnhom", "ខ្ញុំ", "គា"])

    def test_mapping_api(self):
        st, m, _ = self.call("GET", "/api/mapping")
        self.assertEqual((st, m), (200, khdb.default_mapping()))
        m["finals"].append({"latin": "b", "letters": "ប"})
        st, saved, _ = self.call("PUT", "/api/mapping", m)
        self.assertEqual(st, 200)
        self.assertIn({"latin": "b", "letters": "ប"}, saved["finals"])
        bad = copy.deepcopy(m)
        bad["vowels"].pop(0)
        st, err, _ = self.call("PUT", "/api/mapping", bad)
        self.assertEqual(st, 400)
        self.assertTrue(any("inherent" in e for e in err["errors"]))
        st, v, _ = self.call("POST", "/api/mapping/validate", bad)
        self.assertEqual((st, v["ok"]), (200, False))
        self.assertEqual(self.call("GET", "/api/mapping/default")[1], khdb.default_mapping())
        st, hist, _ = self.call("GET", "/api/mapping/history")
        self.assertEqual(st, 200)
        self.assertGreaterEqual(len(hist), 1)
        # the served engine now reads the saved mapping
        self.assertIn('"latin": "b", "letters": "ប"', self.call("GET", "/engine.js")[1])
        self.call("PUT", "/api/mapping", khdb.default_mapping())

    def test_words_api_and_export(self):
        st, row, _ = self.call("POST", "/api/words", {"word": "លាប", "freq": 2, "spellings": "leab leap", "glosses": "paint"})
        self.assertEqual((st, row["added"]), (201, True))
        st, row, _ = self.call("PUT", "/api/words", {"word": "លាប", "glosses": "paint, apply"})
        self.assertEqual((st, row["glosses"]), (200, "paint, apply"))
        st, res, _ = self.call("GET", "/api/words?q=paint")
        self.assertEqual((st, res["total"]), (200, 1))
        self.assertEqual(self.call("POST", "/api/words", {"word": "លាប"})[0], 400)
        st, out, _ = self.call("POST", "/api/export", {})
        self.assertEqual(st, 200)
        self.assertEqual(set(out["written"]), {"data.json", "lexicon.json", "english.json", "mapping.json"})
        with open(os.path.join(self.dir, "lexicon.json"), encoding="utf-8") as f:
            self.assertIn('"លាប": "leab leap"', f.read())
        self.assertEqual(self.call("POST", "/api/export", {"only": ["../x"]})[0], 400)
        st, file, r = self.call("GET", "/api/export/file?name=mapping.json")
        self.assertEqual(st, 200)
        self.assertIn("attachment", r.getheader("Content-Disposition"))
        st, res, _ = self.call("DELETE", "/api/words?word=" + urllib.parse.quote("លាប"))
        self.assertEqual((st, res["result"]), (200, "removed"))
        self.assertEqual(self.call("DELETE", "/api/words?word=" + urllib.parse.quote("លាប"))[0], 404)

    def test_reseed_needs_confirmation(self):
        self.assertEqual(self.call("POST", "/api/seed", {})[0], 400)
        self.assertEqual(self.call("POST", "/api/seed", {"confirm": "replace"})[0], 200)

    def test_safety_checks(self):
        # a foreign Host header (DNS rebinding) is refused
        self.assertEqual(self.call("GET", "/api/stats", headers={"Host": "evil.example"})[0], 403)
        # writes need a JSON content type (a cross-site form cannot send one) and no foreign Origin
        self.assertEqual(self.call("PUT", "/api/mapping", {}, {"Content-Type": "text/plain"})[0], 415)
        self.assertEqual(self.call("POST", "/api/words", {"word": "ខ្ញុំ"}, {"Origin": "https://evil.example"})[0], 403)
        self.assertEqual(self.call("POST", "/api/words", {"word": "ខ្ញុំ"}, {"Origin": "http://localhost:1"})[0], 400)  # allowed origin, but the word exists
        # bad JSON is a 400, not a crash
        c = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        c.request("PUT", "/api/mapping", "{not json", {"Content-Type": "application/json"})
        self.assertEqual(c.getresponse().status, 400)
        self.assertEqual(self.call("GET", "/api/stats")[0], 200, "the server still answers")


if __name__ == "__main__":
    unittest.main()
