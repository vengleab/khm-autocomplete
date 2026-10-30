// Replays the golden vectors (generated from the JS reference) through the real WebAssembly build.
//   sh scripts/build-wasm.sh && node tests/wasm-parity.mjs
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { initSync, WasmEngine, normalize, romKey, romanize, nidaRead } from '../web/pkg/khcore_wasm.js';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const rd = p => fs.readFileSync(path.join(ROOT, p), 'utf8');
const gold = n => JSON.parse(rd('tests/golden/' + n));
initSync({ module: fs.readFileSync(path.join(ROOT, 'web/pkg/khcore_wasm_bg.wasm')) });

let checked = 0, failed = 0, inexact = 0;
const msgs = [];
function check(ok, msg) { checked++; if (!ok) { failed++; if (msgs.length < 15) msgs.push(msg()); } }
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
// probabilities must be bit-identical: wasm float math is deterministic and the port keeps the operation order
const num = (a, b) => { if (a === b) return true; if (typeof a === 'number' && typeof b === 'number' && Math.abs(a - b) <= 1e-12 * Math.max(Math.abs(a), Math.abs(b))) { inexact++; return true; } return false; };
const nulls = x => (x === undefined ? null : x);

for (const c of gold('text.json')) {
  check(normalize(c.s) === c.norm, () => `normalize ${JSON.stringify(c.s)}`);
  check(romanize(c.s) === c.roman, () => `romanize ${JSON.stringify(c.s)}`);
}
for (const c of gold('romkey.json')) check(romKey(c.s) === c.key, () => `romKey ${JSON.stringify(c.s)}`);

function engine(name, data, lex, en) {
  const e = new WasmEngine(data, lex, en);
  const g = gold(`engine_${name}.json`);
  for (const c of g.analyze) {
    const a = JSON.parse(e.analyze(normalize(c.text)));
    check(same(a.tokens, c.tokens) && a.partial === c.partial && a.prev === c.prev, () => `${name} analyze ${JSON.stringify(c.text)}`);
  }
  const cands = (got, want, latin) => got.length === want.length && got.every((a, i) => {
    const b = want[i];
    return a.word === b.word && a.kind === b.kind && (latin || a.append === b.append) && num(a.p, b.p) &&
      (!latin || (a.tier === b.tier && nulls(a.rom) === b.rom && nulls(a.why) === b.why));
  });
  for (const c of g.khmer) {
    const ctx = JSON.parse(e.analyze(normalize(c.text)));
    const prev = c.completed != null ? c.completed : ctx.prev, partial = c.completed != null ? '' : ctx.partial;
    check(cands(JSON.parse(e.khmerCandidates(prev, partial)), c.out, false), () => `${name} khmer ${JSON.stringify(c.text)} completed=${c.completed}`);
  }
  for (const c of g.latin) check(cands(JSON.parse(e.latinCandidates(c.token, c.prev)), c.out, true), () => `${name} latin ${JSON.stringify(c.token)} prev=${c.prev}`);
  for (const c of g.compose) {
    const got = JSON.parse(e.compose(c.t));
    check(got.length === c.out.length && got.every((a, i) => a.word === c.out[i].word && a.why === c.out[i].why && num(a.score, c.out[i].score)), () => `${name} compose ${c.t}`);
  }
  for (const c of g.english) check(cands(JSON.parse(e.englishCandidates(c.token, c.prev)), c.out, true), () => `${name} english ${JSON.stringify(c.token)} prev=${c.prev}`);
  // Auto: the NiDA reading of a run, and the merged candidates with their own spans and sources
  for (const c of g.nida) check(nulls(nidaRead(c.run)) === c.reading, () => `${name} nidaRead ${JSON.stringify(c.run)}`);
  for (const c of g.auto) {
    const got = JSON.parse(e.autoCandidates(c.run, c.prev));
    const ok = got.length === c.out.length && got.every((a, i) => {
      const b = c.out[i];
      return a.word === b.word && a.kind === b.kind && a.append === b.append && a.tier === b.tier && nulls(a.rom) === b.rom && nulls(a.why) === b.why &&
        a.replace === b.replace && nulls(a.source) === b.source && (b.p === null ? a.p === null : num(a.p, b.p));
    });
    check(ok, () => `${name} auto ${JSON.stringify(c.run)} prev=${c.prev}`);
  }
  const KEY = { space: 0, enter: 1, tab: 2 };
  for (const f of [...g.flows, ...g.flowsEn, ...g.flowsAuto]) {
    const mode = f.mode === 'latin' ? 1 : f.mode === 'english' ? 2 : f.mode === 'auto' ? 3 : 0;
    const got = JSON.parse(e.suggest(f.text, mode, f.completed ?? undefined));
    const ok = got.length === f.current.length && got.every((a, i) => {
      const b = f.current[i];
      return a.word === b.word && a.kind === b.kind && a.append === b.append && a.replace === b.replace && nulls(a.roman) === b.rom && nulls(a.why) === b.why && nulls(a.source) === (b.source ?? null) &&
        (b.p === null ? a.prob === null : Math.fround(a.prob) === Math.fround(b.p));
    });
    check(ok, () => `${name} suggest ${JSON.stringify(f.text)} ${f.mode} completed=${f.completed}`);
    for (const [k, code] of Object.entries(KEY)) {
      const act = JSON.parse(e.keyAction(f.text, mode, code, f.completed ?? undefined)), want = f.keys[k];
      check(want === null ? act.action === 'pass' : act.action === 'insert' && act.candidate.word === want.word && act.candidate.replace === want.replace,
        () => `${name} key ${k} ${JSON.stringify(f.text)} ${f.mode}`);
    }
  }
  e.free();
}
engine('seed', rd('data/data.json'), rd('data/lexicon.json'), rd('data/english.json'));
engine('synth', rd('tests/golden/synth_data.json'), rd('tests/golden/synth_lexicon.json'), rd('tests/golden/synth_english.json'));

// a custom mapping (the charts as data) through wasm: rule guesses and the sound index follow it, like the JS reference
{
  const g = gold('engine_mapped.json');
  const e = new WasmEngine(rd('data/data.json'), rd('data/lexicon.json'), rd('data/english.json'), rd('tests/golden/mapping_custom.json'));
  const sameCands = (got, want) => got.length === want.length && got.every((a, i) => {
    const b = want[i];
    return a.word === b.word && a.kind === b.kind && a.tier === b.tier && nulls(a.rom) === b.rom && nulls(a.why) === b.why && (b.p === null ? a.p === null : num(a.p, b.p)) &&
      (b.replace === undefined || a.replace === b.replace) && (b.source === undefined || nulls(a.source) === b.source);
  });
  for (const c of g.compose) {
    const got = JSON.parse(e.compose(c.t));
    check(got.length === c.out.length && got.every((a, i) => a.word === c.out[i].word && a.why === c.out[i].why && num(a.score, c.out[i].score)), () => `mapped compose ${c.t}`);
  }
  for (const c of g.latin) check(sameCands(JSON.parse(e.latinCandidates(c.token, c.prev)), c.out), () => `mapped latin ${JSON.stringify(c.token)}`);
  for (const c of g.auto) check(sameCands(JSON.parse(e.autoCandidates(c.run, c.prev)), c.out), () => `mapped auto ${JSON.stringify(c.run)}`);
  // a bad mapping is an error, not a crash
  let threw2 = false;
  try { new WasmEngine(rd('data/data.json'), rd('data/lexicon.json'), rd('data/english.json'), '{}'); } catch { threw2 = true; }
  check(threw2, () => 'a bad mapping must throw');
  e.free();
}

// the compiled format loads in wasm too (made by khbuild; here we only check bad input is an error, not a crash)
let threw = false;
try { WasmEngine.fromCompiled(new Uint8Array([1, 2, 3])); } catch { threw = true; }
check(threw, () => 'fromCompiled(junk) must throw');

console.log(`wasm parity: ${checked} checks, ${failed} mismatches` + (inexact ? ` (${inexact} probabilities within 1e-12 but not bit-identical)` : ''));
if (failed) { console.error(msgs.join('\n')); process.exit(1); }
