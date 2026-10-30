//! The text service itself: a COM object that TSF creates once per UI thread in every app.
//!
//! Flow (see windows/README.md for the picture):
//! - typing letters, digits or punctuation: keys are never remapped, the app inserts them as typed, TSF tells us
//!   (`OnEndEdit`), we read the text before the caret, ask the engine (Auto mode: it reads the pending run as Khmerlish,
//!   English or keys of the Khmer layout) and show the candidate window;
//! - Space / Enter / Tab: ask the engine what to do; either swallow the key and replace the pending text, or let it pass;
//! - the English state switches all of this off: every key is the app's and the engine is not called.
//!
//! Rules: never hold a `RefCell` borrow across a call into TSF or the engine (those can re-enter us), and wrap every
//! COM entry point in `guard` so a bug here can never unwind into the host app.
use crate::candwin::CandidateWindow;
use crate::edit;
use crate::link::EngineLink;
use crate::logic::{self, Mode, Mods, Plan, Special};
use crate::reg;
use khipc::{Candidate, KeyAction};
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicI32, Ordering};
use windows::core::{implement, ComObject, IUnknown, IUnknownImpl, Interface, Ref, Result, BOOL};
use windows::Win32::Foundation::{E_FAIL, LPARAM, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RMENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfDocumentMgr, ITfEditRecord, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfRange,
    ITfSource, ITfTextEditSink, ITfTextEditSink_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl,
    ITfTextInputProcessor_Impl, ITfThreadMgr, ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl,
    TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_ES_READ, TF_ES_READWRITE, TF_ES_SYNC, TS_SD_READONLY,
};

/// Number of live text-service objects, for `DllCanUnloadNow`.
pub static LIVE_OBJECTS: AtomicI32 = AtomicI32::new(0);

/// Run `f`, turning a panic into `E_FAIL`: nothing may unwind into the host application.
pub fn guard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => Err(E_FAIL.into()),
    }
}

const SYNC_READ: TF_CONTEXT_EDIT_CONTEXT_FLAGS = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READ.0);
const SYNC_WRITE: TF_CONTEXT_EDIT_CONTEXT_FLAGS = TF_CONTEXT_EDIT_CONTEXT_FLAGS(TF_ES_SYNC.0 | TF_ES_READWRITE.0);
/// from the mouse: we are not inside a key handler, so the session may run later
const ASYNC_WRITE: TF_CONTEXT_EDIT_CONTEXT_FLAGS = TF_ES_READWRITE;

/// What to do with a key, decided once in `OnTestKeyDown` and carried out in `OnKeyDown`.
#[derive(Clone)]
enum Decision {
    Pass,
    CycleMode,
    Insert(Candidate),
}

#[implement(ITfTextInputProcessorEx, ITfKeyEventSink, ITfThreadMgrEventSink, ITfTextEditSink)]
pub struct TextService {
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    client_id: Cell<u32>,
    mode: Cell<Mode>,
    /// the word accepted last, valid until a key is typed or the caret moves
    just_accepted: RefCell<Option<String>>,
    /// a collapsed range at the caret right after the accepted word; the caret leaving it means "moved"
    accept_mark: RefCell<Option<ITfRange>>,
    /// the candidates on screen
    shown: RefCell<Vec<Candidate>>,
    link: RefCell<EngineLink>,
    window: RefCell<Option<CandidateWindow>>,
    /// the context candidates were last computed for (the mouse needs it)
    context: RefCell<Option<ITfContext>>,
    thread_sink_cookie: Cell<u32>,
    edit_sink: RefCell<Option<(ITfContext, u32)>>,
    /// (wParam, lParam, decision) from the last OnTestKeyDown
    pending: RefCell<Option<(usize, isize, Decision)>>,
}

impl TextService {
    pub fn new() -> TextService {
        LIVE_OBJECTS.fetch_add(1, Ordering::SeqCst);
        TextService {
            thread_mgr: RefCell::new(None),
            client_id: Cell::new(0),
            mode: Cell::new(Mode::Khmer),
            just_accepted: RefCell::new(None),
            accept_mark: RefCell::new(None),
            shown: RefCell::new(Vec::new()),
            link: RefCell::new(EngineLink::new(reg::dll_dir())),
            window: RefCell::new(None),
            context: RefCell::new(None),
            thread_sink_cookie: Cell::new(0),
            edit_sink: RefCell::new(None),
            pending: RefCell::new(None),
        }
    }
}

impl Drop for TextService {
    fn drop(&mut self) {
        LIVE_OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

fn key_down(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { (GetKeyState(vk.0 as i32) as u16) & 0x8000 != 0 }
}

fn read_mods() -> Mods {
    Mods {
        shift: key_down(VK_SHIFT),
        ctrl: key_down(VK_CONTROL),
        alt: key_down(VK_MENU),
        right_alt: key_down(VK_RMENU),
        win: key_down(VK_LWIN) || key_down(VK_RWIN),
    }
}

impl TextService_Impl {
    // ------------------------------------------------------------------------------------------ state

    fn forget(&self) {
        *self.just_accepted.borrow_mut() = None;
        *self.accept_mark.borrow_mut() = None;
    }

    fn clear_candidates(&self) {
        self.shown.borrow_mut().clear();
        if let Some(w) = self.window.borrow().as_ref() {
            w.hide();
        }
    }

    fn display(c: &Candidate) -> String {
        match c.roman.as_deref() {
            Some(r) if !r.is_empty() => format!("{}  {}", c.word, r),
            _ => c.word.clone(),
        }
    }

    fn show(&self, cands: Vec<Candidate>, anchor: Option<RECT>) {
        let items: Vec<String> = cands.iter().take(6).map(Self::display).collect();
        *self.shown.borrow_mut() = cands;
        if let Some(w) = self.window.borrow().as_ref() {
            w.show(items, anchor);
        }
    }

    // ------------------------------------------------------------------------------------------ reading and suggesting

    /// Recompute the suggestions after an edit, using the (read-only) edit cookie TSF gave us.
    fn refresh(&self, ctx: &ITfContext, ec: u32) {
        *self.context.borrow_mut() = Some(ctx.clone());
        // read-only fields: nothing to suggest
        if let Ok(st) = unsafe { ctx.GetStatus() } {
            if st.dwDynamicFlags & TS_SD_READONLY != 0 {
                self.clear_candidates();
                return;
            }
        }
        // English state: no suggestions, and the engine is not called
        let Some(mode) = self.mode.get().engine_mode() else {
            self.clear_candidates();
            return;
        };
        let mark = self.accept_mark.borrow().clone();
        if let Some(m) = mark {
            if edit::caret_moved_from(ec, ctx, &m).unwrap_or(true) {
                self.forget();
            }
        }
        let text = match edit::read_before_caret(ec, ctx) {
            Ok(Some(t)) => t,
            _ => {
                self.clear_candidates();
                return;
            }
        };
        let just = self.just_accepted.borrow().clone();
        let cands = self.link.borrow_mut().suggest(&text, mode, just.as_deref());
        match cands {
            Some(c) if !c.is_empty() => {
                let rect = edit::caret_rect(ec, ctx).ok().flatten();
                self.show(c, rect);
            }
            _ => self.clear_candidates(),
        }
    }

    // ------------------------------------------------------------------------------------------ keys

    fn decide(&self, ctx: &ITfContext, wparam: WPARAM, _lparam: LPARAM) -> Decision {
        let vk = wparam.0 as u32;
        match logic::plan(self.mode.get(), vk, read_mods()) {
            Plan::PassThrough => Decision::Pass,
            Plan::ForgetAndPassThrough => {
                self.forget();
                Decision::Pass
            }
            Plan::CycleMode => Decision::CycleMode,
            Plan::Ask(k) => self.ask(ctx, k),
        }
    }

    /// Space / Enter / Tab: ask the engine, but only when there is something on screen to accept.
    fn ask(&self, ctx: &ITfContext, key: Special) -> Decision {
        let Some(mode) = self.mode.get().engine_mode() else { return Decision::Pass };
        if self.shown.borrow().is_empty() {
            self.forget();
            return Decision::Pass;
        }
        let (c2, mark) = (ctx.clone(), self.accept_mark.borrow().clone());
        let read = edit::run(ctx, self.client_id.get(), SYNC_READ, move |ec| {
            let text = edit::read_before_caret(ec, &c2)?;
            let moved = match &mark {
                Some(m) => edit::caret_moved_from(ec, &c2, m)?,
                None => false,
            };
            Ok((text, moved))
        });
        let Ok((Some(text), moved)) = read else { return Decision::Pass };
        if moved {
            self.forget();
        }
        let just = self.just_accepted.borrow().clone();
        let action = self.link.borrow_mut().key_action(&text, mode, key as u8, just.as_deref());
        match action {
            Some(KeyAction::Insert(c)) => Decision::Insert(c),
            _ => {
                self.forget();
                Decision::Pass
            }
        }
    }

    /// Carry out a decision. Returns whether the key was eaten.
    fn perform(&self, ctx: &ITfContext, d: Decision) -> bool {
        match d {
            Decision::Pass => false,
            Decision::CycleMode => {
                let m = self.mode.get().next();
                self.mode.set(m);
                reg::save_state(m as u32);
                self.forget();
                self.clear_candidates();
                // tell the user which state this is, next to the caret
                let c2 = ctx.clone();
                let rect =
                    edit::run(ctx, self.client_id.get(), SYNC_READ, move |ec| edit::caret_rect(ec, &c2)).ok().flatten();
                if let Some(w) = self.window.borrow().as_ref() {
                    w.show_label(m.label(), rect);
                }
                true
            }
            Decision::Insert(c) => self.apply(ctx, c, SYNC_WRITE),
        }
    }

    /// Replace the pending text with the candidate's word (no trailing space), and remember it as just accepted.
    /// Returns whether the text was replaced (for an async session that has not run yet: true).
    fn apply(&self, ctx: &ITfContext, c: Candidate, flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS) -> bool {
        let this: ComObject<TextService> = self.to_object();
        let c2 = ctx.clone();
        let word = c.word.clone();
        let r = edit::run(ctx, self.client_id.get(), flags, move |ec| {
            match edit::replace_before_caret(ec, &c2, c.replace_len_utf16, &c.word)? {
                Some(mark) => {
                    *this.just_accepted.borrow_mut() = Some(word);
                    *this.accept_mark.borrow_mut() = Some(mark);
                    Ok(true)
                }
                None => Ok(false),
            }
        });
        match r {
            Ok(done) => done,
            // an async session answers later: there is nothing to report yet
            Err(_) => flags.0 & TF_ES_SYNC.0 == 0,
        }
    }

    /// The user clicked candidate `index` in the window.
    pub fn pick(&self, index: usize) {
        let (ctx, cand) = (self.context.borrow().clone(), self.shown.borrow().get(index).cloned());
        if let (Some(ctx), Some(c)) = (ctx, cand) {
            let _ = self.apply(&ctx, c, ASYNC_WRITE);
        }
    }

    // ------------------------------------------------------------------------------------------ sinks

    fn attach_edit_sink(&self, doc: Option<&ITfDocumentMgr>) {
        self.detach_edit_sink();
        let Some(doc) = doc else { return };
        let Ok(ctx) = (unsafe { doc.GetTop() }) else { return };
        let Ok(src) = ctx.cast::<ITfSource>() else { return };
        let sink: ITfTextEditSink = self.to_interface();
        let Ok(unk) = sink.cast::<IUnknown>() else { return };
        if let Ok(cookie) = unsafe { src.AdviseSink(&ITfTextEditSink::IID, &unk) } {
            *self.edit_sink.borrow_mut() = Some((ctx, cookie));
        }
    }

    fn detach_edit_sink(&self) {
        if let Some((ctx, cookie)) = self.edit_sink.borrow_mut().take() {
            if let Ok(src) = ctx.cast::<ITfSource>() {
                let _ = unsafe { src.UnadviseSink(cookie) };
            }
        }
    }

    fn do_activate(&self, tm: &ITfThreadMgr, tid: u32) -> Result<()> {
        *self.thread_mgr.borrow_mut() = Some(tm.clone());
        self.client_id.set(tid);
        self.mode.set(Mode::from_u32(reg::load_state()));

        let keys: ITfKeystrokeMgr = tm.cast()?;
        let key_sink: ITfKeyEventSink = self.to_interface();
        unsafe { keys.AdviseKeyEventSink(tid, &key_sink, true)? };

        let src: ITfSource = tm.cast()?;
        let thread_sink: ITfThreadMgrEventSink = self.to_interface();
        let unk: IUnknown = thread_sink.cast()?;
        self.thread_sink_cookie.set(unsafe { src.AdviseSink(&ITfThreadMgrEventSink::IID, &unk)? });

        // the candidate window; clicking one comes back to `pick`
        let this: ComObject<TextService> = self.to_object();
        *self.window.borrow_mut() = CandidateWindow::new(Box::new(move |i| this.pick(i))).ok();

        let focus = unsafe { tm.GetFocus() }.ok();
        self.attach_edit_sink(focus.as_ref());
        Ok(())
    }

    fn do_deactivate(&self) -> Result<()> {
        self.detach_edit_sink();
        if let Some(tm) = self.thread_mgr.borrow_mut().take() {
            if let Ok(keys) = tm.cast::<ITfKeystrokeMgr>() {
                let _ = unsafe { keys.UnadviseKeyEventSink(self.client_id.get()) };
            }
            if let Ok(src) = tm.cast::<ITfSource>() {
                let c = self.thread_sink_cookie.replace(0);
                if c != 0 {
                    let _ = unsafe { src.UnadviseSink(c) };
                }
            }
        }
        // dropping the window also drops its click handler, which holds this object: that ends the reference cycle
        *self.window.borrow_mut() = None;
        *self.context.borrow_mut() = None;
        *self.pending.borrow_mut() = None;
        self.shown.borrow_mut().clear();
        self.forget();
        self.client_id.set(0);
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------- COM interfaces

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        guard(|| {
            let tm = ptim.ok()?;
            self.do_activate(tm, tid)
        })
    }
    fn Deactivate(&self) -> Result<()> {
        guard(|| self.do_deactivate())
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(&self, ptim: Ref<ITfThreadMgr>, tid: u32, _flags: u32) -> Result<()> {
        guard(|| {
            let tm = ptim.ok()?;
            self.do_activate(tm, tid)
        })
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, _foreground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        guard(|| {
            let ctx = pic.ok()?;
            let d = self.decide(ctx, wparam, lparam);
            let eaten = !matches!(d, Decision::Pass);
            *self.pending.borrow_mut() = Some((wparam.0, lparam.0, d));
            Ok(BOOL::from(eaten))
        })
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        guard(|| {
            let ctx = pic.ok()?;
            // TSF asks "would you eat this?" first and then delivers it: use the same decision for both
            let cached = self
                .pending
                .borrow_mut()
                .take()
                .filter(|(w, l, _)| *w == wparam.0 && *l == lparam.0)
                .map(|(_, _, d)| d);
            let d = cached.unwrap_or_else(|| self.decide(ctx, wparam, lparam));
            Ok(BOOL::from(self.perform(ctx, d)))
        })
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(BOOL::from(false))
    }
    fn OnKeyUp(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(BOOL::from(false))
    }
    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const windows::core::GUID) -> Result<BOOL> {
        Ok(BOOL::from(false))
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnUninitDocumentMgr(&self, _pdim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnSetFocus(&self, pdimfocus: Ref<ITfDocumentMgr>, _pdimprevfocus: Ref<ITfDocumentMgr>) -> Result<()> {
        guard(|| {
            // another field has the focus: listen to its edits, and drop what belonged to the old one
            self.forget();
            self.clear_candidates();
            *self.context.borrow_mut() = None;
            *self.pending.borrow_mut() = None;
            self.attach_edit_sink(pdimfocus.as_ref());
            Ok(())
        })
    }
    fn OnPushContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
    fn OnPopContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
}

impl ITfTextEditSink_Impl for TextService_Impl {
    fn OnEndEdit(&self, pic: Ref<ITfContext>, ecreadonly: u32, _record: Ref<ITfEditRecord>) -> Result<()> {
        guard(|| {
            if let Some(ctx) = pic.as_ref() {
                self.refresh(ctx, ecreadonly);
            }
            Ok(())
        })
    }
}
