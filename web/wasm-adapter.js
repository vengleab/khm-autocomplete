// Makes the Rust engine (WebAssembly) look like the JS `Engine` the demo UI was written against:
// analyze(text), suggest(ctx), romSuggest(token, ctx), words.length, meta, N.
// Loaded before the template; `wasm_bindgen` (no-modules glue) must already be initialised.
(function () {
  const { WasmEngine } = wasm_bindgen;
  class KhWasmEngine {
    constructor(data) {
      // LEX is the template's lexicon constant; it exists by the time the UI creates an engine
      this.core = new WasmEngine(JSON.stringify(data), JSON.stringify(LEX));
      this.kind = "WebAssembly (Rust)";
      this.words = { length: this.core.vocab() };
      this.meta = { tokens: this.core.tokens(), sentences: this.core.sentences(), source: this.core.source() };
      this.N = this.core.total();
    }
    analyze(text) { return JSON.parse(this.core.analyze(text)); }
    suggest(ctx) { return JSON.parse(this.core.khmerCandidates(ctx.prev, ctx.partial)); }
    romSuggest(token, ctx) {
      return JSON.parse(this.core.latinCandidates(token, ctx.prev)).map(c => ({ ...c, replace: token }));
    }
  }
  window.KH_MAKE_ENGINE = data => new KhWasmEngine(data);
})();
