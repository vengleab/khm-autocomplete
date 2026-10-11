/* A C caller of the khcore C API: opens the seed data, converts "knyom soksabay " the way a shell would. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "khcore.h"

static int failures = 0;
#define CHECK(cond, msg) do { if (!(cond)) { printf("FAIL: %s\n", msg); failures++; } } while (0)

/* Pull "word":"..." out of a JSON result without a JSON library (the real shells use their own parsers). */
static int json_string(const char *json, const char *key, char *out, size_t cap) {
    char pat[64];
    snprintf(pat, sizeof pat, "\"%s\":\"", key);
    const char *p = strstr(json, pat);
    if (!p) return 0;
    p += strlen(pat);
    const char *q = strchr(p, '"');
    if (!q || (size_t)(q - p) >= cap) return 0;
    memcpy(out, p, q - p);
    out[q - p] = 0;
    return 1;
}
static long json_number(const char *json, const char *key) {
    char pat[64];
    snprintf(pat, sizeof pat, "\"%s\":", key);
    const char *p = strstr(json, pat);
    return p ? strtol(p + strlen(pat), NULL, 10) : -1;
}

int main(int argc, char **argv) {
    if (argc != 3) { printf("usage: smoke data.json lexicon.json\n"); return 2; }
    CHECK(kh_abi_version() == KH_ABI_VERSION, "ABI version matches the header");
    KhEngine *e = kh_engine_open(argv[1], argv[2]);
    if (!e) { printf("open failed: %s\n", kh_last_error()); return 1; }

    /* type "knyom soksabay " one character at a time; a Space asks the engine what to do */
    const char *typed = "knyom soksabay ";
    char text[256] = "";
    for (const char *c = typed; *c; c++) {
        if (*c == ' ') {
            char *r = kh_key_action(e, text, KH_MODE_LATIN, KH_KEY_SPACE, NULL);
            CHECK(r != NULL, "key_action returned a result");
            if (r && strstr(r, "\"action\":\"insert\"")) {
                char word[64];
                long n = json_number(r, "replace_len_utf16"); /* ASCII token: UTF-16 units == bytes */
                CHECK(json_string(r, "word", word, sizeof word), "result has a word");
                text[strlen(text) - n] = 0;
                strcat(text, word);
                kh_string_free(r);
                continue; /* the space is swallowed */
            }
            kh_string_free(r);
        }
        size_t l = strlen(text);
        text[l] = *c;
        text[l + 1] = 0;
    }
    CHECK(strcmp(text, "ខ្ញុំសុខសប្បាយ") == 0, "Latin flow gives ខ្ញុំសុខសប្បាយ");
    printf("typed '%s' -> '%s'\n", typed, text);

    char *s = kh_suggest(e, "ខ្ញុំ ស្រ", KH_MODE_KHMER, NULL);
    CHECK(s && strstr(s, "ស្រលាញ់"), "completion for ស្រ");
    kh_string_free(s);

    /* errors are reported, not fatal */
    CHECK(kh_suggest(NULL, "x", KH_MODE_KHMER, NULL) == NULL, "NULL engine gives NULL");
    CHECK(kh_last_error() != NULL, "and an error message");

    kh_engine_free(e);
    printf(failures ? "FAILED\n" : "C smoke test OK\n");
    return failures != 0;
}
