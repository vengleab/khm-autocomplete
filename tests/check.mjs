import { chromium } from 'playwright';
import { fileURLToPath } from 'url';
import path from 'path';

const page_url = 'file://' + path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'web', 'index.html');
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
const pg = await browser.newPage();
const errors = [];
pg.on('pageerror', e => errors.push(e.message));
await pg.route(/fonts\.(googleapis|gstatic)\.com/, r => r.abort());   // fonts are not needed for logic
await pg.goto(page_url);

const result = await pg.evaluate(() => {
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

// typing flow in Latin mode
await pg.click('#m-latin');
await pg.focus('#text');
await pg.keyboard.type('knyom soksabay ');
const typed = await pg.inputValue('#text');
if (typed !== 'ខ្ញុំសុខសប្បាយ') result.push(`Latin flow: got ${typed}`);

if (errors.length) result.push('page errors: ' + errors.join('; '));
await browser.close();
if (result.length) { console.error('FAIL\n' + result.join('\n')); process.exit(1); }
console.log('OK: series charts, rule spelling and typing flow pass');
