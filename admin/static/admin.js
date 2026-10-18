// Helpers shared by the admin pages: API calls, DOM building, notices, and loading the engine from /engine.js.
"use strict";

async function api(method, path, body) {
  const opt = { method, headers: { "Content-Type": "application/json" } };
  if (body !== undefined) opt.body = JSON.stringify(body);
  const res = await fetch(path, opt);
  let data = null;
  try { data = await res.json(); } catch (e) { /* not JSON */ }
  if (!res.ok) {
    const err = new Error((data && data.error) || res.statusText);
    err.errors = (data && data.errors) || [err.message];
    throw err;
  }
  return data;
}

function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (k === "class") el.className = v;
    else if (k === "text") el.textContent = v;
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (v === true) el.setAttribute(k, "");
    else if (v !== false && v != null) el.setAttribute(k, v);
  }
  for (const kid of kids.flat()) if (kid != null) el.append(kid.nodeType ? kid : document.createTextNode(kid));
  return el;
}

let toastTimer = 0;
function toast(text) {
  document.querySelectorAll(".toast").forEach(t => t.remove());
  const t = h("div", { class: "toast", role: "status", text });
  document.body.append(t);
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.remove(), 2600);
}

function showErrors(box, err) {
  box.className = "msg err";
  box.replaceChildren(h("strong", { text: "Not saved" }), h("ul", {}, (err.errors || [err.message]).map(e => h("li", { text: e }))));
  box.hidden = false;
}
function showOk(box, text) { box.className = "msg ok"; box.textContent = text; box.hidden = false; }

function debounce(fn, ms) {
  let t = 0;
  return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); };
}

// Load the JS reference engine built from the current database (window.KhEngine). Resolves when it is ready.
function loadEngine() {
  return new Promise((resolve, reject) => {
    const s = document.createElement("script");
    s.src = "/engine.js?" + Date.now();
    s.onload = () => resolve(window.KhEngine);
    s.onerror = () => reject(new Error("could not load /engine.js"));
    document.head.append(s);
  });
}

function nav(current) {
  const links = [["/", "Start"], ["/mapping.html", "Consonant & vowel mapping"], ["/dictionary.html", "Dictionary"]];
  return h("nav", { class: "tabs", "aria-label": "Admin pages" }, links.map(([href, text]) => h("a", { href, text, "aria-current": href === current ? "page" : null })));
}
