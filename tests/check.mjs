import { chromium } from 'playwright';
import { fileURLToPath } from 'url';
import fs from 'fs';
import path from 'path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
const result = [];
const strips = {};

// One page per engine: index.html runs the JS engine (the reference), index-wasm.html the Rust engine.
// The wasm page only exists after `python3 web/build_demo.py --wasm`; it is checked when present.
const pages = [{ name: 'JS engine', file: 'index.html', wasm: false }];
if (fs.existsSync(path.join(root, 'web', 'index-wasm.html'))) pages.push({ name: 'WASM engine', file: 'index-wasm.html', wasm: true });
else console.log('note: web/index-wasm.html not built, skipping the WASM page (python3 web/build_demo.py --wasm)');

for (const { name, file, wasm } of pages) {
  const pg = await browser.newPage();
  const errors = [];
  pg.on('pageerror', e => errors.push(e.message));
  await pg.route(/fonts\.(googleapis|gstatic)\.com/, r => r.abort());   // fonts are not needed for logic
  await pg.goto('file://' + path.join(root, 'web', file));
  const fail = m => result.push(`[${name}] ${m}`);

  const kind = await pg.evaluate(() => engine.kind || 'JavaScript');
  if (wasm !== kind.startsWith('WebAssembly')) fail(`wrong engine behind the page: ${kind}`);

  const fails = await pg.evaluate(() => {
    const fails = [];
  // first series on ក, second series on គ (romanization column of the vowel chart)
  const A = {"":"ka","ា":"ka","ិ":"ke","ី":"kei","ឹ":"koe","ឺ":"koeu","ុ":"ko","ូ":"kou","ួ":"kuo","ើ":"kaeu","ឿ":"koea","ៀ":"kie","េ":"ke","ែ":"kae","ៃ":"kai","ោ":"kao","ៅ":"kau","ុំ":"kom","ំ":"kam","ាំ":"kam","ះ":"kah"};
  const O = {"":"ko","ា":"kea","ិ":"ki","ី":"ki","ឹ":"koe","ឺ":"keu","ុ":"ku","ូ":"kou","ួ":"kuo","ើ":"keu","ឿ":"keua","ៀ":"kie","េ":"ke","ែ":"kae","ៃ":"key","ោ":"ko","ៅ":"kov","ុំ":"kum","ំ":"kum","ាំ":"koam","ះ":"keah"};
  for (const [s, want] of Object.entries(A)) if (romanize("ក" + s) !== want) fails.push(`first series ក${s}: got ${romanize("ក" + s)}, want ${want}`);
  for (const [s, want] of Object.entries(O)) if (romanize("គ" + s) !== want) fails.push(`second series គ${s}: got ${romanize("គ" + s)}, want ${want}`);
  const C = {ក:"k",ខ:"kh",គ:"k",ឃ:"kh",ង:"ng",ច:"ch",ឆ:"chh",ជ:"ch",ឈ:"chh",ញ:"nh",ដ:"d",ឋ:"th",ឌ:"d",ឍ:"th",ណ:"n",ត:"t",ថ:"th",ទ:"t",ធ:"th",ន:"n",ប:"b",ផ:"ph",ព:"p",ភ:"ph",ម:"m",យ:"y",រ:"r",ល:"l",វ:"v",ស:"s",ហ:"h",ឡ:"l"};
  for (const [l, want] of Object.entries(C)) if (ONSET[l] !== want) fails.push(`consonant ${l}: got ${ONSET[l]}, want ${want}`);
  const has = (t, w) => compose(t, {}).some(c => c.word === w);
  const must = { kea:["គា"], ka:["ក","កា"], gum:["គុំ"], kay:["កៃ"], gaw:["គៅ"], kaw:["កៅ"], ma:["ម៉ា"], mea:["មា"], si:["ស៊ី"], sei:["សី"], keah:["គះ"], kam:["កាំ","កាម"], kuk:["គុក"] };
  for (const [t, ws] of Object.entries(must)) for (const w of ws) if (!has(t, w)) fails.push(`compose(${t}) should include ${w}`);
  for (const [t, w] of [["kea","កា"], ["ma","មា"], ["gum","កុំ"]]) if (has(t, w)) fails.push(`compose(${t}) must not include ${w} (wrong series)`);
  return fails;
  });
  fails.forEach(fail);

  // typing flow in Latin mode: Space converts the word just typed
  await pg.click('#m-latin');
  await pg.focus('#text');
  await pg.keyboard.type('knyom soksabay ');
  const typed = await pg.inputValue('#text');
  if (typed !== 'ខ្ញុំសុខសប្បាយ') fail(`Latin flow: got ${typed}`);

  // Khmer mode flows through the real UI: what the suggestion strip shows, accepting, Tab
  const chips = async () => pg.$$eval('#strip .chip', els => els.map(e => e.textContent.replace(/\s+/g, ' ').trim()));
  const seen = {};
  await pg.click('#m-khmer');
  await pg.fill('#text', 'ខ្ញុំ ចូលចិត្ត ');
  seen.afterVerb = await chips();
  if (!seen.afterVerb.some(c => c.includes('ញ៉ាំ'))) fail(`ញ៉ាំ should be suggested after ចូលចិត្ត: ${seen.afterVerb}`);
  await pg.click('#strip .chip:has-text("ញ៉ាំ")');
  seen.afterAccept = await chips();
  if ((await pg.inputValue('#text')) !== 'ខ្ញុំ ចូលចិត្ត ញ៉ាំ') fail(`accepting ញ៉ាំ gave ${await pg.inputValue('#text')}`);
  if (!seen.afterAccept.some(c => c.includes('បាយ'))) fail(`បាយ should be among the next words: ${seen.afterAccept}`);
  await pg.fill('#text', 'ស្រលាង');
  seen.typo = await chips();
  if (!seen.typo.some(c => c.includes('ស្រលាញ់'))) fail(`typo ស្រលាង should suggest ស្រលាញ់: ${seen.typo}`);
  await pg.fill('#text', 'ស្រ');
  seen.prefix = await chips();
  await pg.focus('#text');
  await pg.keyboard.press('End');
  await pg.keyboard.press('Tab');
  if ((await pg.inputValue('#text')) !== 'ស្រលាញ់') fail(`Tab on ស្រ gave ${await pg.inputValue('#text')}`);
  await pg.fill('#text', '');
  seen.empty = await chips();
  await pg.click('#m-latin');
  for (const t of ['knhom', 'sralanh', 'kea', 'ma']) { await pg.fill('#text', t); seen['latin ' + t] = await chips(); }
  // English -> Khmer mode: Space converts a confident word, a half-typed word is left alone, Tab takes a prefix match
  await pg.click('#m-english');
  await pg.focus('#text');
  await pg.keyboard.type('i love cambodia eating rice wat ');
  const en = await pg.inputValue('#text');
  if (en !== 'ខ្ញុំស្រលាញ់កម្ពុជាញ៉ាំបាយwat ') fail(`English flow: got ${en}`);
  await pg.fill('#text', 'water');
  seen['english water'] = await chips();
  if (!seen['english water'][0]?.includes('ទឹក')) fail(`water should suggest ទឹក first: ${seen['english water']}`);
  for (const t of ['eating', 'teachers', 'watr', 'good']) { await pg.fill('#text', t); seen['english ' + t] = await chips(); }
  await pg.fill('#text', 'wat');
  await pg.focus('#text');
  await pg.keyboard.press('End');
  await pg.keyboard.press('Tab');
  if ((await pg.inputValue('#text')) !== 'ទឹក') fail(`Tab on "wat" gave ${await pg.inputValue('#text')}`);
  // Auto mode: the one Khmer state. Khmerlish, an English word and keys of the Khmer layout in one sentence.
  await pg.click('#m-auto');
  if ((await pg.getAttribute('#m-auto', 'aria-pressed')) !== 'true') fail('Auto mode button is not pressed after a click');
  await pg.fill('#text', '');
  await pg.focus('#text');
  await pg.keyboard.type('knhom chong water xjJuM ');
  const auto = await pg.inputValue('#text');
  if (auto !== 'ខ្ញុំចង់ទឹកខ្ញុំ') fail(`Auto flow: got ${auto}`);
  // punctuation before the run stays: each candidate replaces its own span
  await pg.fill('#text', '');
  await pg.keyboard.type('hello,water ');
  if ((await pg.inputValue('#text')) !== 'hello,ទឹក') fail(`Auto span: got ${await pg.inputValue('#text')}`);
  // unknown English stays as typed, with the two chips; Space leaves it alone
  await pg.fill('#text', 'windows');
  seen['auto windows'] = await chips();
  if (!seen['auto windows'][0]?.includes('Khmer (NiDA)') || !seen['auto windows'][1]?.includes('English')) fail(`windows should offer layout / raw chips: ${seen['auto windows']}`);
  await pg.focus('#text');
  await pg.keyboard.press('End');
  await pg.keyboard.type(' ');
  if ((await pg.inputValue('#text')) !== 'windows ') fail(`Space must leave unknown English alone: ${await pg.inputValue('#text')}`);
  // clicking the raw chip keeps the text and ends the run; clicking the layout chip reads it as Khmer
  await pg.fill('#text', 'windows');
  await pg.click('#strip .chip:has-text("English")');
  if ((await pg.inputValue('#text')) !== 'windows') fail(`raw chip changed the text: ${await pg.inputValue('#text')}`);
  await pg.fill('#text', 'windows');
  await pg.click('#strip .chip:has-text("Khmer (NiDA)")');
  if ((await pg.inputValue('#text')) !== 'ឹិនដោឹស') fail(`layout chip gave ${await pg.inputValue('#text')}`);
  // a half-typed Khmerlish word is not converted by Space; Tab takes the first suggestion; Enter never takes a chip
  await pg.fill('#text', '');
  await pg.keyboard.type('phnom ');
  if ((await pg.inputValue('#text')) !== 'phnom ') fail(`Auto: Space converted a prefix: ${await pg.inputValue('#text')}`);
  await pg.fill('#text', 'phnom');
  seen['auto phnom'] = await chips();
  await pg.keyboard.press('Tab');
  if ((await pg.inputValue('#text')) !== 'ភ្នំពេញ') fail(`Auto Tab on phnom gave ${await pg.inputValue('#text')}`);
  await pg.fill('#text', 'windows');
  await pg.keyboard.press('Enter');
  if ((await pg.inputValue('#text')) !== 'windows\n') fail(`Enter must stay a newline over layout / raw chips: ${JSON.stringify(await pg.inputValue('#text'))}`);
  await pg.fill('#text', 'windows');
  await pg.keyboard.press('Tab');
  if ((await pg.inputValue('#text')) !== 'ឹិនដោឹស') fail(`Auto Tab over layout / raw chips gave ${await pg.inputValue('#text')}`);
  // keys of the Khmer layout, partly typed: completions, then the word
  await pg.fill('#text', 'xjJ');
  seen['auto xjJ'] = await chips();
  if (!seen['auto xjJ'][0]?.includes('ខ្ញុំ')) fail(`xjJ should complete to ខ្ញុំ: ${seen['auto xjJ']}`);
  for (const t of ['knhom', 'wat', 'hello,water', 'sralan', '123']) { await pg.fill('#text', t); seen['auto ' + t] = await chips(); }
  // text that ends in Khmer: next words, completions and typo fixes as in Khmer mode
  await pg.fill('#text', 'ខ្ញុំ ចូលចិត្ត ');
  seen['auto after verb'] = await chips();
  if (!seen['auto after verb'].some(c => c.includes('ញ៉ាំ'))) fail(`Auto: ញ៉ាំ should follow ចូលចិត្ត: ${seen['auto after verb']}`);
  await pg.fill('#text', 'ស្រលាង');
  seen['auto typo'] = await chips();
  if (!seen['auto typo'].some(c => c.includes('ស្រលាញ់'))) fail(`Auto: typo ស្រលាង should suggest ស្រលាញ់: ${seen['auto typo']}`);
  // the page starts in Auto
  await pg.reload();
  if ((await pg.getAttribute('#m-auto', 'aria-pressed')) !== 'true') fail('the page should start in Auto mode');
  strips[name] = seen;

  if (errors.length) fail('page errors: ' + errors.join('; '));
  await pg.close();
}

// both engines must show exactly the same suggestions in the real UI
if (strips['JS engine'] && strips['WASM engine']) {
  for (const k of Object.keys(strips['JS engine'])) {
    const a = JSON.stringify(strips['JS engine'][k]), b = JSON.stringify(strips['WASM engine'][k]);
    if (a !== b) result.push(`UI parity "${k}": JS ${a} vs WASM ${b}`);
  }
}
await browser.close();
if (result.length) { console.error('FAIL\n' + result.join('\n')); process.exit(1); }
console.log(`OK on ${pages.map(p => p.name).join(' + ')}: series charts, rule spelling, typing flows (JS, Latin, English, Auto)` + (pages.length > 1 ? ', and identical suggestions in the UI' : ''));
