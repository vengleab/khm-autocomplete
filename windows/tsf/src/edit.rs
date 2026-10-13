//! TSF edit sessions: reading the text before the caret and replacing it. All text in TSF is UTF-16.
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use windows::core::{implement, Result, BOOL};
use windows::Win32::Foundation::{E_FAIL, RECT};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfEditSession_Impl, ITfRange, TF_AE_NONE, TF_ANCHOR_END,
    TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_DEFAULT_SELECTION, TF_SELECTION, TF_SELECTIONSTYLE,
};

/// How many UTF-16 units before the caret are read: enough for the last few words.
pub const CONTEXT_UNITS: usize = 96;

type Job = Box<dyn FnOnce(u32) -> Result<()>>;

#[implement(ITfEditSession)]
struct Session {
    job: RefCell<Option<Job>>,
}

impl ITfEditSession_Impl for Session_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        crate::tip::guard(|| match self.job.borrow_mut().take() {
            Some(job) => job(ec),
            None => Ok(()),
        })
    }
}

/// Run `f` inside an edit session on `ctx` and return its result. With `TF_ES_SYNC` (allowed inside key handlers)
/// the answer is there when this returns; for async sessions the result is `Err(E_FAIL)` until it has run.
pub fn run<T: 'static>(
    ctx: &ITfContext,
    client_id: u32,
    flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    f: impl FnOnce(u32) -> Result<T> + 'static,
) -> Result<T> {
    let slot: Rc<RefCell<Option<Result<T>>>> = Rc::new(RefCell::new(None));
    let slot2 = slot.clone();
    let session: ITfEditSession = Session {
        job: RefCell::new(Some(Box::new(move |ec| {
            *slot2.borrow_mut() = Some(f(ec));
            Ok(())
        }))),
    }
    .into();
    unsafe { ctx.RequestEditSession(client_id, &session, flags)? }.ok()?;
    let r = slot.borrow_mut().take();
    r.unwrap_or(Err(E_FAIL.into()))
}

fn new_selection(range: Option<ITfRange>) -> TF_SELECTION {
    TF_SELECTION {
        range: ManuallyDrop::new(range),
        style: TF_SELECTIONSTYLE { ase: TF_AE_NONE, fInterimChar: BOOL(0) },
    }
}

/// The current selection as a range, or `None` when the context has none.
pub fn selection(ec: u32, ctx: &ITfContext) -> Result<Option<ITfRange>> {
    let mut sel = [new_selection(None)];
    let mut fetched = 0u32;
    unsafe { ctx.GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &mut fetched)? };
    if fetched == 0 {
        return Ok(None);
    }
    Ok(unsafe { ManuallyDrop::take(&mut sel[0].range) })
}

/// The text before the caret (at most `CONTEXT_UNITS` UTF-16 units). `None` when text is selected, or no caret.
pub fn read_before_caret(ec: u32, ctx: &ITfContext) -> Result<Option<String>> {
    let Some(caret) = selection(ec, ctx)? else { return Ok(None) };
    unsafe {
        if !caret.IsEmpty(ec)?.as_bool() {
            return Ok(None); // a selection: leave the field alone
        }
        let before = caret.Clone()?;
        let mut moved = 0i32;
        before.ShiftStart(ec, -(CONTEXT_UNITS as i32), &mut moved, std::ptr::null())?;
        if moved == 0 {
            return Ok(Some(String::new()));
        }
        let mut buf = [0u16; CONTEXT_UNITS];
        let mut got = 0u32;
        before.GetText(ec, 0, &mut buf, &mut got)?;
        Ok(Some(String::from_utf16_lossy(&buf[..got as usize])))
    }
}

/// Replace the `replace_len` UTF-16 units before the caret with `text` (0 = insert at the caret) and put the caret
/// after it. Returns a collapsed range at the new caret, which stays put (backward gravity) until the user edits.
/// `Ok(None)` when the text could not be replaced (selection, or fewer characters than expected).
pub fn replace_before_caret(ec: u32, ctx: &ITfContext, replace_len: usize, text: &str) -> Result<Option<ITfRange>> {
    use windows::Win32::UI::TextServices::TF_GRAVITY_BACKWARD;
    let Some(caret) = selection(ec, ctx)? else { return Ok(None) };
    unsafe {
        if !caret.IsEmpty(ec)?.as_bool() {
            return Ok(None);
        }
        let target = caret.Clone()?;
        if replace_len > 0 {
            let mut moved = 0i32;
            target.ShiftStart(ec, -(replace_len as i32), &mut moved, std::ptr::null())?;
            if moved != -(replace_len as i32) {
                return Ok(None);
            }
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        target.SetText(ec, 0, &wide)?;
        target.Collapse(ec, TF_ANCHOR_END)?;
        let mut sel = [new_selection(Some(target.clone()))];
        let r = ctx.SetSelection(ec, &sel);
        ManuallyDrop::drop(&mut sel[0].range);
        r?;
        let mark = target.Clone()?;
        mark.SetGravity(ec, TF_GRAVITY_BACKWARD, TF_GRAVITY_BACKWARD)?;
        Ok(Some(mark))
    }
}

/// Screen rectangle of the caret, for placing the candidate window.
pub fn caret_rect(ec: u32, ctx: &ITfContext) -> Result<Option<RECT>> {
    let Some(caret) = selection(ec, ctx)? else { return Ok(None) };
    unsafe {
        let view = ctx.GetActiveView()?;
        let mut rc = RECT::default();
        let mut clipped = BOOL(0);
        view.GetTextExt(ec, &caret, &mut rc, &mut clipped)?;
        Ok(Some(rc))
    }
}

/// Has the caret moved away from `mark` (a range made by `replace_before_caret`)?
pub fn caret_moved_from(ec: u32, ctx: &ITfContext, mark: &ITfRange) -> Result<bool> {
    let Some(caret) = selection(ec, ctx)? else { return Ok(true) };
    unsafe { Ok(caret.CompareStart(ec, mark, windows::Win32::UI::TextServices::TF_ANCHOR_START)? != 0) }
}
