#!/usr/bin/env python3
"""A small local web server for the two admin pages (standard library only, listens on 127.0.0.1).

  python3 admin/server.py            # then open http://127.0.0.1:8765/
  python3 admin/server.py --port 9000 --db /path/to/khmer.db --data /path/to/data

  /                 start page
  /mapping.html     edit the consonant / vowel mapping, try it live
  /dictionary.html  the Khmer word dictionary: Khmerlish spellings (lexicon) and English glosses (translation)
  /engine.js        the JS reference engine (web/template.html, engine part) built from the current database
  /api/...          JSON API used by the pages (see handle_api)

The database (data/khmer.db) is created from data/*.json on first start. "Export" writes data/mapping.json, lexicon.json,
english.json and data.json back; rebuild the engine data with khbuild (see admin/README.md).

Safety: the server only listens on localhost, answers only requests whose Host is localhost, and requires a JSON content
type for every write, so another web page cannot change your data from your browser. There is no login: do not expose the port.
"""
import argparse
import json
import mimetypes
import os
import sys
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import khdb  # noqa: E402

HERE = khdb.HERE
STATIC = os.path.join(HERE, "static")
TEMPLATE = os.path.join(khdb.ROOT, "web", "template.html")
UI_ANCHOR = "/* -------------------------------------------------------------------------- UI */"
PAGES = {"/": "index.html", "/index.html": "index.html", "/mapping.html": "mapping.html", "/dictionary.html": "dictionary.html", "/admin.css": "admin.css", "/admin.js": "admin.js"}
MAX_BODY = 8 * 1024 * 1024


def engine_js(conn):
    """The engine part of web/template.html, filled with the current database and wrapped so the pages can use it:
    `window.KhEngine` = { Engine, normalize, romKey, romanize, compose, setMapping, SEED, LEX, EN, MAP, tables() }."""
    with open(TEMPLATE, encoding="utf-8") as f:
        html = f.read()
    script = html[html.index("<script>") + len("<script>"): html.rindex("</script>")]
    if script.count(UI_ANCHOR) != 1:
        raise RuntimeError("the UI anchor of web/template.html is missing or not unique")
    part = script.split(UI_ANCHOR)[0]
    texts = khdb.export_texts(conn)
    part = part.replace("__DATA__", texts["data.json"]).replace("__LEX__", texts["lexicon.json"]).replace("__EN__", texts["english.json"]).replace("__MAP__", texts["mapping.json"])
    api = (
        "\nwindow.KhEngine = { Engine, normalize, clusters, romKey, romanize, compose, enKey, nidaRead, setMapping, SEED, LEX, EN, MAP,"
        " tables() { return { CONS, VOW, FINAL, INDEP, ONSET_LAT, FINAL_LAT, TO_A, TO_O }; } };\n"
    )
    return "(function () {\n" + part + api + "})();\n"


class Handler(BaseHTTPRequestHandler):
    server_version = "KhmerAdmin/1"
    db_path = khdb.DEFAULT_DB
    data_dir = khdb.DATA

    # no query strings in the log: they hold what the user searched for
    def log_message(self, fmt, *args):
        sys.stderr.write("%s %s\n" % (self.command, urllib.parse.urlsplit(self.path).path))

    # ---------------------------------------------------------------- helpers
    def _host_ok(self):
        host = (self.headers.get("Host") or "").rsplit(":", 1)[0].strip("[]").lower()
        return host in ("127.0.0.1", "localhost", "::1")

    def _send(self, status, body, ctype, extra=None):
        if isinstance(body, str):
            body = body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _json(self, status, obj):
        self._send(status, json.dumps(obj, ensure_ascii=False), "application/json; charset=utf-8")

    def _error(self, status, msg, errors=None):
        self._json(status, {"error": msg, "errors": errors or [msg]})

    def _body(self):
        n = int(self.headers.get("Content-Length") or 0)
        if n > MAX_BODY:
            raise khdb.ValidationError("request too large")
        raw = self.rfile.read(n) if n else b""
        try:
            return json.loads(raw.decode("utf-8")) if raw else {}
        except (ValueError, UnicodeDecodeError):
            raise khdb.ValidationError("the request body is not valid JSON")

    def _conn(self):
        conn = khdb.connect(self.db_path)
        if not khdb.is_seeded(conn):
            khdb.seed(conn, self.data_dir)
        return conn

    # ---------------------------------------------------------------- dispatch
    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        self._dispatch()

    def do_POST(self):
        self._dispatch()

    def do_PUT(self):
        self._dispatch()

    def do_DELETE(self):
        self._dispatch()

    def _dispatch(self):
        if not self._host_ok():
            return self._error(403, "this server answers requests for localhost only")
        url = urllib.parse.urlsplit(self.path)
        path = url.path
        query = urllib.parse.parse_qs(url.query, keep_blank_values=True)
        writing = self.command in ("POST", "PUT", "DELETE")
        if writing:
            if not (self.headers.get("Content-Type") or "").lower().startswith("application/json"):
                return self._error(415, "writes must be sent as application/json")
            origin = self.headers.get("Origin")
            if origin and urllib.parse.urlsplit(origin).hostname not in ("127.0.0.1", "localhost", "::1"):
                return self._error(403, "cross-site writes are refused")
        try:
            if path.startswith("/api/"):
                return self.handle_api(path, query)
            if self.command not in ("GET", "HEAD"):
                return self._error(405, "method not allowed")
            if path == "/engine.js":
                conn = self._conn()
                try:
                    return self._send(200, engine_js(conn), "application/javascript; charset=utf-8")
                finally:
                    conn.close()
            if path in PAGES:
                with open(os.path.join(STATIC, PAGES[path]), "rb") as f:
                    body = f.read()
                ctype = mimetypes.guess_type(PAGES[path])[0] or "application/octet-stream"
                return self._send(200, body, ctype + ("; charset=utf-8" if ctype.startswith("text/") or "javascript" in ctype else ""))
            return self._error(404, "not found")
        except khdb.ValidationError as e:
            return self._error(400, str(e), e.errors)
        except BrokenPipeError:
            pass
        except Exception as e:  # a bug must not kill the server; say so, without echoing any typed text
            sys.stderr.write("internal error: %s: %s\n" % (type(e).__name__, e))
            return self._error(500, "internal error: " + type(e).__name__)

    # ---------------------------------------------------------------- API
    def handle_api(self, path, q):
        one = lambda name, default="": (q.get(name) or [default])[0]
        conn = self._conn()
        try:
            cmd = self.command
            if path == "/api/stats" and cmd == "GET":
                return self._json(200, khdb.stats(conn))
            # ---- mapping
            if path == "/api/mapping" and cmd == "GET":
                return self._json(200, khdb.get_mapping(conn))
            if path == "/api/mapping" and cmd == "PUT":
                m = self._body()
                khdb.set_mapping(conn, m)
                return self._json(200, khdb.get_mapping(conn))
            if path == "/api/mapping/default" and cmd == "GET":
                return self._json(200, khdb.default_mapping())
            if path == "/api/mapping/validate" and cmd == "POST":
                errs = khdb.validate_mapping(self._body())
                return self._json(200, {"ok": not errs, "errors": errs})
            if path == "/api/mapping/history" and cmd == "GET":
                return self._json(200, khdb.mapping_history(conn))
            if path == "/api/mapping/version" and cmd == "GET":
                m = khdb.mapping_version(conn, int(one("id", "0") or 0))
                return self._json(200, m) if m else self._error(404, "no such version")
            # ---- dictionary
            if path == "/api/words" and cmd == "GET":
                return self._json(200, khdb.list_words(conn, one("q"), one("filter", "all"), int(one("offset", "0") or 0), int(one("limit", "50") or 50)))
            if path == "/api/words" and cmd in ("POST", "PUT"):
                b = self._body()
                row = khdb.save_word(conn, b.get("word"), b.get("freq"), b.get("spellings"), b.get("glosses"), create=(cmd == "POST"))
                return self._json(200 if cmd == "PUT" else 201, row)
            if path == "/api/words" and cmd == "DELETE":
                what = khdb.delete_word(conn, one("word"))
                return self._json(200, {"result": what}) if what else self._error(404, "no such word")
            # ---- files
            if path == "/api/export" and cmd == "POST":
                only = self._body().get("only") or None
                names = set(khdb.export_texts(conn))
                if only and not set(only) <= names:
                    raise khdb.ValidationError("unknown file in `only`")
                return self._json(200, {"written": khdb.export_files(conn, self.data_dir, only), "folder": os.path.abspath(self.data_dir)})
            if path == "/api/export/file" and cmd == "GET":
                name = one("name")
                texts = khdb.export_texts(conn)
                if name not in texts:
                    return self._error(404, "no such file")
                return self._send(200, texts[name], "application/json; charset=utf-8", {"Content-Disposition": 'attachment; filename="%s"' % name})
            if path == "/api/seed" and cmd == "POST":
                if self._body().get("confirm") != "replace":
                    raise khdb.ValidationError('send {"confirm": "replace"}: this replaces every edit in the database with the JSON files')
                return self._json(200, khdb.seed(conn, self.data_dir, force=True))
            return self._error(404, "no such API call")
        finally:
            conn.close()


def make_server(host="127.0.0.1", port=8765, db=khdb.DEFAULT_DB, data=khdb.DATA):
    handler = type("BoundHandler", (Handler,), {"db_path": db, "data_dir": data})
    return ThreadingHTTPServer((host, port), handler)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--db", default=khdb.DEFAULT_DB)
    ap.add_argument("--data", default=khdb.DATA, help="folder of data.json / lexicon.json / english.json / mapping.json")
    a = ap.parse_args(argv)
    conn = khdb.connect(a.db)
    if not khdb.is_seeded(conn):
        print("creating %s from %s" % (a.db, a.data))
        khdb.seed(conn, a.data)
    conn.close()
    srv = make_server("127.0.0.1", a.port, a.db, a.data)
    print("Khmer admin on http://127.0.0.1:%d/  (database %s)  Ctrl+C to stop" % (a.port, a.db))
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        print()
    return 0


if __name__ == "__main__":
    main()
