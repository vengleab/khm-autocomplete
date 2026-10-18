// Drives the two admin pages in a real browser against a real local server (temp database, temp data folder):
//   CHROMIUM_PATH=... node tests/admin-check.mjs
import { chromium } from 'playwright';
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import fs from 'fs';
import os from 'os';
import path from 'path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'khadmin-ui-'));
for (const f of ['data.json', 'lexicon.json', 'english.json', 'mapping.json']) fs.copyFileSync(path.join(root, 'data', f), path.join(tmp, f));
const PORT = 8700 + Math.floor(Math.random() * 200);
const srv = spawn('python3', [path.join(root, 'admin', 'server.py'), '--port', String(PORT), '--db', path.join(tmp, 't.db'), '--data', tmp], { stdio: ['ignore', 'pipe', 'pipe'] });
const base = `http://127.0.0.1:${PORT}`;
const failures = [];
const fail = m => failures.push(m);
const cleanup = () => { srv.kill(); fs.rmSync(tmp, { recursive: true, force: true }); };

try {
  for (let i = 0; i < 50; i++) { try { if ((await fetch(base + '/api/stats')).ok) break; } catch { /* not up yet */ } await new Promise(r => setTimeout(r, 100)); }
  const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const pg = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  pg.on('pageerror', e => errors.push(e.message));
  pg.on('console', m => { if (m.type() === 'error' && !/fonts\.(googleapis|gstatic)|Failed to load resource/.test(m.text())) errors.push(m.text()); });
  await pg.route(/fonts\.(googleapis|gstatic)\.com/, r => r.abort());
  pg.on('dialog', d => d.accept());

  // ---- start page
  await pg.goto(base + '/');
  await pg.waitForFunction(() => /277 words/.test(document.getElementById('s-dict').textContent));
  if (!/33 consonants/.test(await pg.textContent('#s-map'))) fail('start page: mapping stats');

  // ---- mapping page
  await pg.goto(base + '/mapping.html');
  await pg.waitForFunction(() => document.getElementById('state').textContent === 'saved');
  if ((await pg.locator('#t-cons tbody tr').count()) !== 33) fail('33 consonant rows expected');
  if ((await pg.locator('#t-vow tbody tr').count()) !== 21) fail('21 vowel rows expected');
  await pg.fill('#try-kh', 'លាប');
  await pg.waitForFunction(() => document.getElementById('out-kh').textContent === 'leap');
  await pg.fill('#try-lat', 'leab');
  await pg.waitForFunction(() => /No rule guess/.test(document.getElementById('out-lat').textContent));
  // add the final b -> ប: leab becomes a rule guess for លាប, live, before saving
  await pg.click('#add-fin');
  const last = pg.locator('#t-fin tbody tr').last();
  await last.locator('input').nth(0).fill('b');
  await last.locator('input').nth(1).fill('ប');
  await pg.waitForFunction(() => document.getElementById('state').textContent === 'unsaved changes');
  await pg.waitForFunction(() => /លាប/.test(document.getElementById('out-lat').textContent), null, { timeout: 5000 }).catch(() => fail('leab should offer លាប with the final b'));
  // the generated spelling follows the Final column: ប finals written "b" make លាប read "leab"
  await pg.locator('#t-cons tbody tr').nth(20).locator('input.lat').nth(1).fill('b');   // ប is the 21st consonant
  await pg.waitForFunction(() => document.getElementById('out-kh').textContent === 'leab', null, { timeout: 5000 }).catch(() => fail('លាប should read leab after changing the final of ប'));
  // an invalid edit is reported and blocks Save
  await pg.locator('#t-vow tbody tr').first().locator('input').first().fill('ក');
  await pg.waitForFunction(() => /problem/.test(document.getElementById('state').textContent));
  if (!(await pg.isDisabled('#save'))) fail('Save must be disabled while the mapping has problems');
  if (!/inherent/.test(await pg.textContent('#msg'))) fail('the problem should name the missing inherent vowel');
  await pg.locator('#t-vow tbody tr').first().locator('input').first().fill('');
  await pg.waitForFunction(() => document.getElementById('state').textContent === 'unsaved changes');
  await pg.click('#save');
  await pg.waitForFunction(() => document.getElementById('state').textContent === 'saved');
  await pg.screenshot({ path: path.join(tmp, 'mapping.png') });
  await pg.reload();
  await pg.waitForFunction(() => document.getElementById('state').textContent === 'saved');
  if ((await pg.locator('#t-fin tbody tr').count()) !== 14) fail('the saved final should survive a reload (14 final rows)');
  // revert, history and reset
  await pg.locator('#t-cons tbody tr').first().locator('input.lat').first().fill('q');
  await pg.click('#revert');
  await pg.waitForFunction(() => document.getElementById('state').textContent === 'saved');
  await pg.click('#reset');
  await pg.waitForFunction(() => document.getElementById('state').textContent === 'unsaved changes');
  if ((await pg.locator('#t-fin tbody tr').count()) !== 13) fail('the built-in default has 13 finals');
  await pg.click('#revert');
  await pg.click('#export');
  await pg.waitForFunction(() => /mapping.json/.test(document.getElementById('msg').textContent));
  const exported = JSON.parse(fs.readFileSync(path.join(tmp, 'mapping.json'), 'utf8'));
  if (!exported.finals.some(f => f.latin === 'b' && f.letters === 'ប')) fail('exported mapping.json lacks the new final');

  // ---- dictionary page
  await pg.goto(base + '/dictionary.html');
  await pg.waitForFunction(() => document.querySelectorAll('#t tbody tr').length === 25);
  await pg.fill('#a-word', 'លាប');
  await pg.click('#a-suggest');
  if ((await pg.inputValue('#a-sp')) !== 'leab') fail('the suggested spelling should follow the saved mapping (leab)');
  await pg.fill('#a-gl', 'paint, apply');
  await pg.fill('#a-freq', '4');
  await pg.click('#add button[type=submit]');
  await pg.waitForFunction(() => /1 added here/.test(document.getElementById('stats').textContent));
  await pg.waitForFunction(() => document.querySelectorAll('#t tbody tr').length === 1);
  // the try box finds the new word by its spelling and by its translation
  await pg.fill('#try', 'leab');
  await pg.waitForFunction(() => /លាប/.test(document.getElementById('out').textContent));
  await pg.check('input[name=kind][value=english]');
  await pg.fill('#try', 'paint');
  await pg.waitForFunction(() => /លាប/.test(document.getElementById('out').textContent));
  // edit a row and save it
  const row = pg.locator('#t tbody tr').first();
  await row.locator('input').nth(1).fill('leab leap lib');
  if (!(await pg.evaluate(() => document.querySelector('#t tbody tr').classList.contains('dirty')))) fail('an edited row should be marked');
  await row.getByRole('button', { name: 'Save' }).click();
  await pg.waitForFunction(() => /saved/.test(document.querySelector('#t tbody tr td:last-child').textContent));
  // a bad spelling is reported on the row
  await row.locator('input').nth(1).fill('bad_spelling!');
  await row.getByRole('button', { name: 'Save' }).click();
  await pg.waitForFunction(() => /letters/.test(document.querySelector('#t tbody tr td:last-child').textContent));
  // filters and search
  await pg.fill('#q', '');
  await pg.selectOption('#filter', 'no_gloss');
  await pg.waitForFunction(() => document.getElementById('range').textContent.includes('of 4'));
  await pg.selectOption('#filter', 'all');
  await pg.fill('#q', 'water');
  await pg.waitForFunction(() => document.querySelectorAll('#t tbody tr').length >= 1 && /ទឹក/.test(document.getElementById('t').textContent));
  await pg.screenshot({ path: path.join(tmp, 'dictionary.png') });
  await pg.click('#export');
  await pg.waitForFunction(() => /lexicon.json/.test(document.getElementById('msg').textContent));
  const lex = fs.readFileSync(path.join(tmp, 'lexicon.json'), 'utf8');
  if (!lex.includes('"លាប": "leab leap lib"')) fail('exported lexicon.json lacks the edited word');
  const data = JSON.parse(fs.readFileSync(path.join(tmp, 'data.json'), 'utf8'));
  if (data.words.at(-1) !== 'លាប' || data.freq.at(-1) !== 4 || data.meta.vocab !== 278) fail('exported data.json lacks the added word');
  if (process.env.KEEP_SHOTS) { fs.copyFileSync(path.join(tmp, 'mapping.png'), process.env.KEEP_SHOTS + '/mapping.png'); fs.copyFileSync(path.join(tmp, 'dictionary.png'), process.env.KEEP_SHOTS + '/dictionary.png'); }
  if (errors.length) fail('page errors: ' + errors.join('; '));
  await browser.close();
} catch (e) {
  fail('exception: ' + (e.stack || e));
} finally {
  cleanup();
}
if (failures.length) { console.error('FAIL\n' + failures.join('\n')); process.exit(1); }
console.log('OK: admin pages (mapping edit / live try / validation / save / history / export, dictionary add / edit / search / try / export)');
