// Golden-vector generator: runs the JS reference (web/template.html, unmodified) in a Node vm and writes
// tests/golden/*.json. The Rust tests in core/tests must reproduce these outputs.
//
//   node tests/golden/gen.mjs
//
// Everything is deterministic (seeded PRNG), so re-running must leave `git status` clean.
// The UI part of the template (refresh/accept/commitLatin) needs a DOM, so the few lines of it that carry
// engine behavior are restated below (refreshJS, commitLatinJS, keyJS); tests/check.mjs covers the real UI.
// Auto mode needs no restating: autoResolve / autoKey are DOM-free and live in the engine part of the template, so the
// `nida`, `auto` and `flowsAuto` sections run the reference as it is. They use their own PRNG, so older vectors never move.
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

function loadRef(dataText, lexText, enText) {
  const code = enginePart.replace('__DATA__', () => dataText).replace('__LEX__', () => lexText).replace('__EN__', () => enText) +
    '\n({normalize, clusters, romKey, romanize, compose, Engine, lev, levS, enKey, enStems, CONS, VOW, FINAL, INDEP, ONSET_LAT, FINAL_LAT, TO_A, TO_O, VOWSIGNS, LEX, EN, SEED, NIDA, nidaRead, autoResolve, autoKey});';
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

const seedRef = loadRef(read('data/data.json'), read('data/lexicon.json'), read('data/english.json'));
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
// synthetic English glossary for the big data set: pseudo-words, a few phrases, inflected forms in the glosses
function makeSynthEnglish(words) {
  const r = mulberry32(31337), n = k => Math.floor(r() * k), p = a => a[n(a.length)];
  const ON = ['b', 'c', 'd', 'f', 'g', 'h', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't', 'w', 'st', 'tr', 'ch', 'sh', 'th', ''];
  const VW = ['a', 'e', 'i', 'o', 'u', 'ai', 'ea', 'oo', 'ou'], CO = ['', 'n', 't', 'r', 's', 'l', 'm', 'd', 'ck', 'ng'];
  const word = () => { let w = ''; for (let i = 1 + n(3); i > 0; i--) w += p(ON) + p(VW) + p(CO); return w; };
  const en = {};
  for (let t = 0; t < 7000; t++) {
    const w = words[Math.min(words.length - 1, Math.floor(Math.pow(r(), 1.4) * words.length))];
    const gl = [];
    for (let i = 1 + n(3); i > 0; i--) {
      let g = word();
      if (r() < 0.25) g += ' ' + word();
      if (r() < 0.2) g = g + p(['ing', 'ed', 's', 'es']);
      if (r() < 0.05) g = g.toUpperCase();
      if (r() < 0.05) g += "'s";
      gl.push(g);
    }
    en[w] = gl.join(', ');
  }
  return en;
}
const synth = makeSynth();
const synthEnText = JSON.stringify(makeSynthEnglish(synth.data.words));
fs.writeFileSync(path.join(HERE, 'synth_english.json'), synthEnText + '\n');
const synthDataText = JSON.stringify(synth.data), synthLexText = JSON.stringify(synth.lex);
fs.writeFileSync(path.join(HERE, 'synth_data.json'), synthDataText + '\n');
fs.writeFileSync(path.join(HERE, 'synth_lexicon.json'), synthLexText + '\n');
const synthRef = loadRef(synthDataText, synthLexText, synthEnText);

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
  nida: seedRef.NIDA,
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
  const tok = mode !== 'khmer' && completedWord == null ? (LATIN.exec(before) || [''])[0] : '';
  let ctx, current;
  if (tok) {
    ctx = E.analyze(before.slice(0, before.length - tok.length));
    if (ctx.partial) { ctx.tokens.push(ctx.partial); ctx.prev = ctx.partial; ctx.partial = ''; }
    ctx.token = tok;
    current = mode === 'english' ? E.enSuggest(tok, ctx) : E.romSuggest(tok, ctx);
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
    if (mode === 'khmer') return null;
    const m = LATIN.exec(text); if (!m) return null;
    const pre = ref.normalize(text.slice(0, text.length - m[0].length));
    const ctx = E.analyze(pre); if (ctx.partial) ctx.prev = ctx.partial;
    const top = mode === 'english' ? E.enSuggest(m[0], ctx).find((c, i) => i === 0 && c.tier < 1) : E.romSuggest(m[0], ctx)[0]; if (!top) return null;
    return { word: top.word, replace: m[0] };
  }
  const r = refreshJS(ref, text, mode, completedWord);
  if (!r.current.length) return null;
  const pending = mode !== 'khmer' && LATIN.test(text);
  if (key === 'tab' || (key === 'enter' && pending)) return acceptInfo(ref, text, completedWord, r.ctx, r.current[0]);
  return null;
}
const cleanCand = c => ({ word: c.word, kind: c.kind, append: !!c.append, p: c.p, tier: c.tier === undefined ? null : c.tier, rom: c.rom === undefined ? null : c.rom, why: c.why === undefined ? null : c.why });

const ON_E = ['b', 'd', 'k', 'm', 'p', 's', 't', 'st', 'sh', ''], VW_E = ['a', 'e', 'i', 'o', 'u', 'ea', 'oo'], CO_E = ['', 'n', 't', 'r', 's', 'l', 'ck'];
const pick2 = (a, rp) => rp(a);
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
  // ---- English mode (own PRNG: adding it must not move any existing vector)
  const RE = mulberry32(555 + words.length), re = k => Math.floor(RE() * k), rp = a => a[re(a.length)];
  const glosses = [...new Set(Object.values(ref.EN).flatMap(v => v.split(',')).map(g => g.trim()).filter(Boolean))];
  // the typo scan is slow in the JS reference, so the big synthetic set gets fewer tokens than the seed set
  const cap = glosses.length > 2000 ? 110 : 900;
  const pool = glosses.length > cap ? glosses.filter((_, i) => i % Math.ceil(glosses.length / cap) === 0) : glosses;
  const etoks = [];
  const mut = x => { const i = re(x.length + 1); return [x.slice(0, i) + x.slice(i + 1), x.slice(0, i) + rp([...'abcdeghiklmnoprstuvy']) + x.slice(i), x.slice(0, i) + rp([...'abcdeghiklmnoprstuvy']) + x.slice(i + 1)][re(3)]; };
  for (const g of pool) {
    const w1 = g.split(' ')[0];
    etoks.push(g, g.toUpperCase(), w1, g.slice(0, 1 + re(g.length)), mut(g), w1 + rp(['ing', 'ed', 's', 'es', 'ly']), w1.replace(/e$/, '') + 'ing', w1 + w1[w1.length - 1] + 'ed', w1.replace(/y$/, 'ies'));
  }
  etoks.push('water', 'eating', 'teachers', 'wat', 'watr', 'hello', 'xyzq', 'i', 'a', "o'clock", 'good-morning', 'Good', '', '-', "'", 'THANK', 'happier', 'stopped', 'cities', 'the');
  for (let i = 0; i < 80; i++) etoks.push(Array.from({ length: 1 + re(3) }, () => rp(ON_E) + rp(VW_E) + rp(CO_E)).join(''));
  const english = etoks.map(tk => {
    const t = rp(['', '', pick2(sents, rp).split(' ').slice(0, 2).join(' ') + ' ']);
    const ctx = E.analyze(ref.normalize(t)); if (ctx.partial) { ctx.tokens.push(ctx.partial); ctx.prev = ctx.partial; ctx.partial = ''; }
    return { text: t, token: tk, prev: ctx.prev, out: E.enSuggest(tk, ctx).map(cleanCand) };
  });
  const flowsEn = [];
  const ftexts = [];
  for (const tk of etoks.filter((_, i) => i % 7 === 0).slice(0, glosses.length > 2000 ? 40 : 120)) { ftexts.push(tk, tk + ' ', pick2(sents, rp).split(' ').slice(0, 1 + re(2)).join(' ') + ' ' + tk.replace(/[^A-Za-z'-]/g, '')); }
  ftexts.push('water', 'eating', 'wat', 'thank', 'ខ្ញុំ water', 'xyzq', '');
  for (const t of ftexts) {
    const r2 = refreshJS(ref, t, 'english', null);
    flowsEn.push({
      text: t, mode: 'english', completed: null,
      current: r2.current.map(c => ({ ...cleanCand(c), replace: r2.tok ? r2.tok : acceptInfo(ref, t, null, r2.ctx, c).replace })),
      keys: Object.fromEntries(['space', 'enter', 'tab'].map(k => [k, keyJS(ref, t, 'english', k, null)])),
    });
  }
  // ---- Auto mode (its own PRNG again: adding it must not move any existing vector)
  const RA = mulberry32(9091 + words.length), ra = k => Math.floor(RA() * k), rq = a => a[ra(a.length)];
  const INV = {};                      // Khmer text -> the key that types it (first key wins)
  for (const [k, v] of Object.entries(ref.NIDA)) if (!(v in INV)) INV[v] = k;
  const keysOf = w => { let o = ''; for (const ch of w) { if (INV[ch] === undefined) return null; o += INV[ch]; } return o; };
  const ASCII = Array.from({ length: 94 }, (_, i) => String.fromCharCode(33 + i));
  const asciiOnly = x => x.replace(/[^\x21-\x7e]/g, '');
  const garbage = () => Array.from({ length: 1 + ra(7) }, () => rq(ASCII)).join('');
  const runs = [...ASCII];
  for (let i = 0; i < 100; i++) runs.push(garbage());
  for (const t of toks.filter((_, i) => i % 5 === 0)) runs.push(asciiOnly(t));
  for (const t of etoks.filter((_, i) => i % 9 === 0)) runs.push(asciiOnly(t));
  for (const w of words) {
    const cl = ref.clusters(w), k = keysOf(w);
    if (!k) continue;
    runs.push(k, keysOf(cl.slice(0, 1 + ra(cl.length)).join('')), keysOf(w + rq(words)) || k, k + rq(ASCII), rq([',', '.', '(', '1']) + k);
  }
  runs.push('knhom', 'xjJuM', 'hello,water', 'don\'t', 'a-b', '1', '123', '12ab', '"', "'", '-', 'ABC', 'Knhom', 'Water', 'xjJuM.', '៛');
  const clean = c => ({ word: c.word, kind: c.kind, append: !!c.append, tier: c.tier === undefined ? null : c.tier, p: c.p === undefined ? null : c.p, rom: c.rom === undefined ? null : c.rom, why: c.why === undefined ? null : c.why, replace: c.replace, source: c.source });
  const nida = [...new Set(runs.filter(Boolean))].map(run => ({ run, reading: ref.nidaRead(run) }));
  const auto = runs.filter(Boolean).map(run => {
    const t = rq(['', '', rq(sents).split(' ').slice(0, 2).join(' ') + ' ', rq(sents).replace(/ /g, '').slice(0, 9)]);
    const ctx = E.analyze(ref.normalize(t)); if (ctx.partial) { ctx.tokens.push(ctx.partial); ctx.prev = ctx.partial; ctx.partial = ''; }
    return { text: t, run, prev: ctx.prev, out: E.autoSuggest(run, ctx).map(clean) };
  });
  const flowsAuto = [];
  const atexts = [];
  for (const run of runs.filter((_, i) => i % 3 === 0)) atexts.push(rq(['', '', rq(sents).split(' ').slice(0, 1 + ra(2)).join(' ') + ' ', rq(sents).replace(/ /g, '').slice(0, 7), 'hello,']) + run);
  for (const s of sents.slice(0, 30)) { const ws = s.split(' '); atexts.push(ws.slice(0, 1 + ra(ws.length)).join(' ') + ' ', ws.slice(0, 1 + ra(ws.length)).join('')); }
  for (const w of words.slice(0, 40)) atexts.push(w, ref.clusters(w).slice(0, 2).join(''));
  atexts.push('', ' ', 'knhom', 'knhom ', 'water', 'hello,water', 'ខ្ញុំ knhom', 'ខ្ញុំknhom', 'ខ្ញុំ\u200bwater', 'ខ្ញុំ ចូលចិត្ត ', 'ខ្ញុំ ចូលចិត្ត ញ៉ាំ', '123', '.', 'knhom.', 'xjJuM', 'xjJuMcDM', 'ខ្ញុំ។ knhom');
  const keyOut = (text, key, completed) => {
    const c = ref.autoKey(E, text, key, completed);
    return c ? acceptInfo(ref, text, key === 'space' ? null : completed, null, c) : null;
  };
  for (const t of atexts) {
    for (const completed of [null, ...(ra(6) === 0 ? [rq(words)] : [])]) {
      const r = ref.autoResolve(E, t, completed);
      flowsAuto.push({
        text: t, mode: 'auto', completed, run: r.run,
        current: r.current.map(c => ({ ...clean(c), replace: acceptInfo(ref, t, completed, r.ctx, c).replace, source: c.source === undefined ? null : c.source })),
        keys: Object.fromEntries(['space', 'enter', 'tab'].map(k => [k, keyOut(t, k, completed)])),
      });
    }
  }
  write(`engine_${name}.json`, { name, analyze, khmer, latin, compose: comp, flows, english, flowsEn, nida, auto, flowsAuto });
}

const synthWords = synth.data.words;
const synthSentences = Array.from({ length: 150 }, () => Array.from({ length: 2 + ri(4) }, () => synthWords[Math.floor(Math.pow(R(), 2) * 6000)]).join(' '));
engineGolden('seed', seedRef, seedWords, sentences, lexVals);
engineGolden('synth', synthRef, Array.from({ length: 120 }, () => synthWords[Math.floor(Math.pow(R(), 1.3) * 21000)]), synthSentences, Object.values(synth.lex).flatMap(v => v.split(/\s+/)).filter((_, i) => i % 40 === 0));
console.log('golden files written to', HERE);
