// Golden-vector generator: runs the JS reference (web/template.html, unmodified) in a Node vm and writes
// tests/golden/*.json. The Rust tests in core/tests must reproduce these outputs.
//
//   node tests/golden/gen.mjs
//
// Everything is deterministic (seeded PRNG), so re-running must leave `git status` clean.
// The UI part of the template (refresh/accept/commitLatin) needs a DOM, so the few lines of it that carry
// engine behavior are restated below (refreshJS, commitLatinJS, keyJS); tests/check.mjs covers the real UI.
import fs from 'fs';
import vm from 'vm';
import path from 'path';
import { fileURLToPath } from 'url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const read = p => fs.readFileSync(path.join(ROOT, p), 'utf8');
const write = (name, obj) => fs.writeFileSync(path.join(HERE, name), JSON.stringify(obj) + '\n');

// ------------------------------------------------------------------ load the reference
const html = read('web/template.html');
const script = html.match(/<script>([\s\S]*)<\/script>/)[1];
const ANCHOR = '/* -------------------------------------------------------------------------- UI */';
if (script.split(ANCHOR).length !== 2) throw new Error('UI anchor not unique in template.html');
const enginePart = script.split(ANCHOR)[0];

function loadRef(dataText, lexText) {
  const code = enginePart.replace('__DATA__', () => dataText).replace('__LEX__', () => lexText) +
    '\n({normalize, clusters, romKey, romanize, compose, Engine, lev, levS, CONS, VOW, FINAL, INDEP, ONSET_LAT, FINAL_LAT, TO_A, TO_O, VOWSIGNS, LEX, SEED});';
  const ref = vm.runInNewContext(code);
  ref.E = new ref.Engine(JSON.parse(dataText));
  return ref;
}

// ------------------------------------------------------------------ deterministic randomness
function mulberry32(a) {
  return function () {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const R = mulberry32(20240611);
const ri = n => Math.floor(R() * n);
const pick = a => a[ri(a.length)];
const chance = p => R() < p;

const seedRef = loadRef(read('data/data.json'), read('data/lexicon.json'));
const LETTERS = seedRef.CONS.map(c => c[0]);
const SIGNS_X = ['៉', '៊', '់', '័', 'ំ', 'ះ', '៍', '៑', '៏', 'ៗ'];
const INVIS = ['​', '‌', '‍', '⁠', '﻿'];
const VS = [...seedRef.VOWSIGNS];

// ------------------------------------------------------------------ synthetic bigger data set
function makeSynth() {
  const r = mulberry32(777), n = k => Math.floor(r() * k), p = a => a[n(a.length)];
  const words = new Set(), list = [];
  while (list.length < 21000) {
    let w = '';
    const k = 1 + (r() < 0.45 ? 0 : r() < 0.6 ? 1 : r() < 0.7 ? 2 : 3);
    for (let i = 0; i < k; i++) {
      w += p(LETTERS);
      if (r() < 0.2) w += '្' + p(LETTERS.filter(l => 'កខគឃងចឆជឈញដឋឌឍណតថទធនបផពភមយរលវសហឡអ'.includes(l)));
      if (r() < 0.6) w += p(VS);
      if (r() < 0.12) w += p(['ំ', 'ះ', '់', '៉']);
    }
    w = seedRef.normalize(w);
    if (!words.has(w)) { words.add(w); list.push(w); }
  }
  const freq = list.map((_, i) => Math.floor(200000 / Math.pow(i + 1, 1.05)) + 1);
  const bi = {};
  const row = (m) => { const seen = new Map(); for (let t = 0; t < m; t++) seen.set(Math.floor(Math.pow(r(), 2) * 6000), 1 + n(20)); return [...seen].flat(); };
  bi['^'] = row(30);
  for (let i = 0; i < 4000; i++) if (r() < 0.9) bi[String(i)] = row(3 + n(10));
  const meta = { sentences: 8000, tokens: freq.reduce((a, b) => a + b, 0), vocab: list.length, source: 'synthetic (golden tests)' };
  const lex = {};
  const subs = [['kh', 'k'], ['ou', 'u'], ['ea', 'ia'], ['nh', 'ny'], ['ei', 'ey'], ['oe', 'u'], ['ch', 'j'], ['ph', 'p'], ['th', 't']];
  for (let i = 0; i < 4000; i++) {
    const w = list[Math.min(list.length - 1, Math.floor(Math.pow(r(), 1.5) * 9000))];
    const base = seedRef.romanize(w);
    const sp = [base];
    for (let t = n(3); t > 0; t--) { const [a, b] = p(subs); sp.push(base.replace(a, b)); }
    if (r() < 0.2) sp[0] = sp[0].replace(/(.{2,3})/, '$1-');
    if (r() < 0.1) sp.push(sp[0].toUpperCase());
    lex[w] = sp.join(' ');
  }
  return { data: { meta, words: list, freq, bi }, lex };
}
const synth = makeSynth();
const synthDataText = JSON.stringify(synth.data), synthLexText = JSON.stringify(synth.lex);
fs.writeFileSync(path.join(HERE, 'synth_data.json'), synthDataText + '\n');
fs.writeFileSync(path.join(HERE, 'synth_lexicon.json'), synthLexText + '\n');
const synthRef = loadRef(synthDataText, synthLexText);

// ------------------------------------------------------------------ input generators
function fuzzCluster() {
  let c = chance(0.04) ? pick([...'ឥឦឧឩឪឫឬឭឮឯឰឱឲឳ']) : pick(LETTERS);
  const marks = [];
  for (let t = ri(4); t > 0; t--) {
    const k = R();
    if (k < 0.3) marks.push('្' + pick(LETTERS));
    else if (k < 0.65) marks.push(pick(VS));
    else if (k < 0.97) marks.push(pick(SIGNS_X));
    else marks.push('្');
  }
  let s = c + marks.join('');
  if (chance(0.05)) s += pick(INVIS);
  return s;
}
function fuzzKhmer() {
  let s = '';
  for (let t = 1 + ri(4); t > 0; t--) s += fuzzCluster();
  if (chance(0.08)) s = pick(['a', 'k', '1', ' ', '។', 'é']) + s;
  if (chance(0.05)) s += pick(['​', ' ', 'x']);
  return s;
}
const ONS = ['k', 'kh', 'g', 'ng', 'ch', 'j', 'chh', 'nh', 'ny', 'd', 'th', 'n', 't', 'b', 'ph', 'p', 'm', 'y', 'r', 'l', 'v', 'w', 's', 'h', 'q', 'x', 'c', ''];
const VOWS = ['a', 'aa', 'e', 'ei', 'ey', 'oe', 'oeu', 'eu', 'o', 'ou', 'u', 'oo', 'uo', 'ua', 'aeu', 'ae', 'oea', 'eua', 'ie', 'ia', 'ea', 'ai', 'ay', 'ao', 'au', 'aw', 'ov', 'om', 'am', 'um', 'oam', 'eam', 'ah', 'eah', 'i', 'ii', 'ee'];
const FINS = ['', '', '', 'k', 'ng', 'ch', 'nh', 't', 'n', 'p', 'm', 'y', 'l', 'v', 's', 'h', 'r'];
function fuzzLatin() {
  let s = '';
  for (let t = 1 + ri(3); t > 0; t--) s += pick(ONS) + pick(VOWS) + pick(FINS);
  if (chance(0.15)) s = s[0].toUpperCase() + s.slice(1);
  if (chance(0.06)) s = s.replace(/a/, pick(['ā', 'á', 'ə', 'ɛ', 'ɔ', 'ɑ', 'ɨ', 'Ə', 'Ɛ', 'Ɔ']));
  if (chance(0.08)) s = s.slice(0, 2) + pick(["-", "'", ' ', '1']) + s.slice(2);
  return s;
}
function mutate(s) {
  const k = ri(4), i = ri(s.length + 1), L = 'abcdeghiklmnoprstuvy';
  if (!s) return s;
  if (k === 0) return s.slice(0, i) + s.slice(i + 1);
  if (k === 1) return s.slice(0, i) + pick([...L]) + s.slice(i);
  if (k === 2) return s.slice(0, i) + pick([...L]) + s.slice(i + 1);
  return s.slice(0, i) + s.slice(i + 1, i + 2) + s.slice(i, i + 1) + s.slice(i + 2);
}
const seedWords = JSON.parse(read('data/data.json')).words;
const sentences = read('data/seed_corpus.txt').split('\n').filter(Boolean);
const lexVals = Object.values(JSON.parse(read('data/lexicon.json'))).flatMap(v => v.split(/\s+/));

// ------------------------------------------------------------------ text-level golden: normalize, clusters, romanize, romKey
{
  const inputs = [...seedWords, ...sentences, ...sentences.map(s => s.replace(/ /g, '')), ...lexVals];
  for (const w of seedWords) {
    // the same word with marks shuffled / invisibles added / NFD
    const sh = [...w].reverse().join('');
    inputs.push(sh, w.split('').join('​'), w.normalize('NFD'));
  }
  for (let i = 0; i < 2500; i++) inputs.push(fuzzKhmer());
  for (const s of ['', ' ', 'មា៉', 'ម៉ា', 'មុ៉', 'ក្', 'ក្ា', 'ក្ឣ', 'ា', 'abc ក', 'ក😀ខ', '១២៣', 'ខ្ញុំ', 'ក្ក្ខា៉']) inputs.push(s);
  write('text.json', inputs.map(s => ({ s, norm: seedRef.normalize(s), clusters: seedRef.clusters(s), roman: seedRef.romanize(s) })));

  const lat = [...lexVals, ...seedWords.map(w => seedRef.romanize(w)), ...Array.from({ length: 2500 }, fuzzLatin),
    'Ṅ', 'KNYOM', 'knyom', 'khnhom', 'Ə', 'ə', 'ɛ', 'ā', 'ē', 'x', 'xyz', '', '123', 'gn', 'ng', 'rr', 'hh', 'yy', 'bay', 'ray', 'rah', 'chhh', 'wuo', 'AEU', 'İstanbul'];
  write('romkey.json', lat.map(s => ({ s, key: seedRef.romKey(s) })));

  const lv = [];
  for (let i = 0; i < 600; i++) { const a = fuzzLatin().toLowerCase().replace(/[^a-z]/g, ''), b = mutate(a); lv.push({ a, b, d: seedRef.levS(a, b) }); }
  const lc = [];
  for (let i = 0; i < 600; i++) { const a = seedRef.clusters(fuzzKhmer()), b = chance(0.5) ? a.slice(0, ri(a.length + 1)) : seedRef.clusters(fuzzKhmer()); lc.push({ a, b, d: seedRef.lev(a, b) }); }
  write('lev.json', { latin: lv, clusters: lc });
}

// ------------------------------------------------------------------ tables
write('tables.json', {
  cons: seedRef.CONS, vow: seedRef.VOW, final: seedRef.FINAL, indep: seedRef.INDEP,
  onsetLat: Object.entries(seedRef.ONSET_LAT), finalLat: Object.entries(seedRef.FINAL_LAT),
  toA: [...seedRef.TO_A], toO: [...seedRef.TO_O], vowsigns: [...seedRef.VOWSIGNS],
});

// ------------------------------------------------------------------ ln (segmentation costs depend on it)
{
  const hex = x => { const b = Buffer.alloc(8); b.writeDoubleBE(x); return b.toString('hex'); };
  const xs = [];
  const sd = JSON.parse(read('data/data.json'));
  const sumS = sd.freq.reduce((a, b) => a + b, 0), sumY = synth.data.freq.reduce((a, b) => a + b, 0);
  sd.freq.forEach(f => xs.push(f / sumS));
  synth.data.freq.forEach(f => xs.push(f / sumY));
  for (let i = 0; i < 4000; i++) xs.push(R());
  write('ln.json', xs.map(x => [hex(x), hex(Math.log(x))]));
}

// ------------------------------------------------------------------ engine-level golden
const LATIN = /[A-Za-z'\-]+$/;
function refreshJS(ref, text, mode, completedWord) {
  const E = ref.E, before = ref.normalize(text);
  const tok = mode === 'latin' && completedWord == null ? (LATIN.exec(before) || [''])[0] : '';
  let ctx, current;
  if (tok) {
    ctx = E.analyze(before.slice(0, before.length - tok.length));
    if (ctx.partial) { ctx.tokens.push(ctx.partial); ctx.prev = ctx.partial; ctx.partial = ''; }
    ctx.token = tok;
    current = E.romSuggest(tok, ctx);
  } else {
    ctx = completedWord != null ? { prev: completedWord, partial: '', tokens: E.analyze(before).tokens } : E.analyze(before);
    current = E.suggest(ctx);
  }
  return { ctx, current, tok };
}
const acceptInfo = (ref, text, completedWord, ctx, c) =>
  ({ word: c.word, replace: c.replace !== undefined ? c.replace : c.append ? '' : completedWord != null ? '' : ref.E.analyze(ref.normalize(text)).partial });
function keyJS(ref, text, mode, key, completedWord) {
  const E = ref.E;
  if (key === 'space') {            // commitLatin: the text before the caret does not include the space
    if (mode !== 'latin') return null;
    const m = LATIN.exec(text); if (!m) return null;
    const pre = ref.normalize(text.slice(0, text.length - m[0].length));
    const ctx = E.analyze(pre); if (ctx.partial) ctx.prev = ctx.partial;
    const top = E.romSuggest(m[0], ctx)[0]; if (!top) return null;
    return { word: top.word, replace: m[0] };
  }
  const r = refreshJS(ref, text, mode, completedWord);
  if (!r.current.length) return null;
  const pending = mode === 'latin' && LATIN.test(text);
  if (key === 'tab' || (key === 'enter' && pending)) return acceptInfo(ref, text, completedWord, r.ctx, r.current[0]);
  return null;
}
const cleanCand = c => ({ word: c.word, kind: c.kind, append: !!c.append, p: c.p, tier: c.tier === undefined ? null : c.tier, rom: c.rom === undefined ? null : c.rom, why: c.why === undefined ? null : c.why });

function engineGolden(name, ref, words, sents, latinPool) {
  const E = ref.E;
  const texts = [];
  for (const s of sents) {
    texts.push(s, s + ' ', s.replace(/ /g, ''), s.replace(/ /g, '') + ' ');
    const cl = ref.clusters(s);
    for (let t = 0; t < 3; t++) { const k = 1 + ri(cl.length); texts.push(cl.slice(0, k).join('')); }
    const ws = s.split(' ');
    for (let t = 0; t < 2; t++) { const k = 1 + ri(ws.length); texts.push(ws.slice(0, k).join(' ') + ' ' + (chance(0.5) ? cl[ri(cl.length)] : '')); }
  }
  for (const w of words) {
    texts.push(w, w + ' ');
    const cl = ref.clusters(w);
    for (let k = 1; k <= cl.length; k++) texts.push(cl.slice(0, k).join(''));
    // typos: drop, swap and replace one cluster
    if (cl.length >= 2) { const i = ri(cl.length); texts.push(cl.filter((_, j) => j !== i).join('')); texts.push(cl.map((c, j) => (j === i ? pick(LETTERS) : c)).join('')); texts.push([...cl.slice(0, -1), pick(LETTERS)].join('')); }
  }
  for (let i = 0; i < 200; i++) texts.push(fuzzKhmer());
  texts.push('', ' ', '។ ', 'ខ្ញុំ។ ', 'ខ្ញុំ\nសូម', 'សួស្តី ! ខ្ញុំ', 'ខ្ញុំ  ចូលចិត្ត  ', 'abc', 'ខ្ញុំabc');

  const analyze = texts.map(t => { const n = ref.normalize(t), c = E.analyze(n); return { text: t, tokens: c.tokens, partial: c.partial, prev: c.prev }; });
  const khmer = [];
  for (const t of texts) {
    const n = ref.normalize(t), ctx = E.analyze(n);
    khmer.push({ text: t, completed: null, out: E.suggest(ctx).map(cleanCand) });
    if (chance(0.25) && ctx.tokens.length) { const w = pick(ctx.tokens); khmer.push({ text: t, completed: w, out: E.suggest({ prev: w, partial: '', tokens: ctx.tokens }).map(cleanCand) }); }
  }

  const latin = [];
  const toks = [];
  for (const x of latinPool) { toks.push(x, x.toLowerCase(), x.slice(0, 1 + ri(x.length)), mutate(x), mutate(mutate(x))); }
  for (let i = 0; i < 150; i++) toks.push(fuzzLatin());
  toks.push('knyom', 'orkun', 'phnom', 'penh', 'srolanh', 'kea', 'ma', 'gum', 'k', 'a', 'sabay', 'sabai', "bong-", 'xyzq', 'KNHOM');
  for (const tk of toks) {
    const t = pick(['', '', '', pick(sents).split(' ').slice(0, 2).join(' ') + ' ', pick(sents).replace(/ /g, '').slice(0, 9)]);
    const pre = ref.normalize(t);
    const ctx = E.analyze(pre); if (ctx.partial) { ctx.tokens.push(ctx.partial); ctx.prev = ctx.partial; ctx.partial = ''; }
    latin.push({ text: t, token: tk, prev: ctx.prev, out: E.romSuggest(tk, ctx).map(cleanCand) });
  }

  const comp = [...toks, ...Array.from({ length: 60 }, fuzzLatin), 'kea', 'ka', 'gum', 'kay', 'gaw', 'kaw', 'ma', 'mea', 'si', 'sei', 'keah', 'kam', 'kuk', 'a', 'ae', 'ng', 'ngam', 'nham', 'bay', ''].map(t => ({ t, out: ref.compose(t, E.prior) }));

  // end-to-end flows: what the shells call
  const flows = [];
  const flowTexts = [];
  for (const s of sents.slice(0, 40)) { const ws = s.split(' '); flowTexts.push(ws.slice(0, 1 + ri(ws.length)).join(' ') + ' '); flowTexts.push(ws.slice(0, 1 + ri(ws.length)).join('')); }
  for (const w of words.slice(0, 60)) { flowTexts.push(w); flowTexts.push(ref.clusters(w).slice(0, 2).join('')); }
  for (let i = 0; i < 60; i++) { const a = pick(sents).split(' ').slice(0, ri(3)).join(' '); flowTexts.push((a ? a + ' ' : '') + pick(toks).replace(/[^A-Za-z'-]/g, '')); }
  flowTexts.push('', 'knyom', 'knyom ', 'ខ្ញុំ ចូលចិត្ត ', 'ខ្ញុំ ចូលចិត្ត ញ៉ាំ', 'ខ្ញុំ knhom', 'ស្រលាង', 'xyzq', "bong-");
  for (const t of flowTexts) {
    for (const mode of ['khmer', 'latin']) {
      for (const completed of [null, ...(chance(0.15) && !LATIN.test(t) ? [pick(words)] : [])]) {
        const r = refreshJS(ref, t, mode, completed);
        flows.push({
          text: t, mode, completed,
          current: r.current.map(c => ({ ...cleanCand(c), replace: r.tok ? r.tok : acceptInfo(ref, t, completed, r.ctx, c).replace })),
          keys: Object.fromEntries(['space', 'enter', 'tab'].map(k => [k, keyJS(ref, t, mode, k, completed)])),
        });
      }
    }
  }
  write(`engine_${name}.json`, { name, analyze, khmer, latin, compose: comp, flows });
}

const synthWords = synth.data.words;
const synthSentences = Array.from({ length: 150 }, () => Array.from({ length: 2 + ri(4) }, () => synthWords[Math.floor(Math.pow(R(), 2) * 6000)]).join(' '));
engineGolden('seed', seedRef, seedWords, sentences, lexVals);
engineGolden('synth', synthRef, Array.from({ length: 120 }, () => synthWords[Math.floor(Math.pow(R(), 1.3) * 21000)]), synthSentences, Object.values(synth.lex).flatMap(v => v.split(/\s+/)).filter((_, i) => i % 40 === 0));
console.log('golden files written to', HERE);
