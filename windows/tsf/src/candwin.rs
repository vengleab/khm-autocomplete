//! The candidate window: a small popup next to the caret. It never takes the focus (so the host app keeps
//! typing), draws with DirectWrite (Khmer shaping and font fallback come from the system) and reports clicks.
use std::cell::{Cell, RefCell};
use windows::core::{w, Result};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS,
    DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, GetSysColor, InvalidateRect, MonitorFromPoint, COLOR_GRAYTEXT,
    COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    PAINTSTRUCT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, KillTimer, LoadCursorW, RegisterClassExW,
    SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, CS_DROPSHADOW, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW,
    MA_NOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_PAINT,
    WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows_numerics::Vector2;

const LABEL_TIMER: usize = 1;
const LABEL_MS: u32 = 1300;
const FONT_PX: f32 = 18.0;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 6.0;
const GAP: f32 = 2.0;

struct Item {
    layout: IDWriteTextLayout,
    x: f32,
    w: f32,
}

struct State {
    on_pick: Box<dyn Fn(usize)>,
    d2d: ID2D1Factory,
    dwrite: IDWriteFactory,
    format: RefCell<Option<(IDWriteTextFormat, u32)>>,
    items: RefCell<Vec<Item>>,
    /// a mode label is not a list of candidates: clicking it does nothing
    is_label: Cell<bool>,
    target: RefCell<Option<ID2D1HwndRenderTarget>>,
    px_size: Cell<(u32, u32)>,
    scale: Cell<f32>,
}

pub struct CandidateWindow {
    hwnd: HWND,
    state: Box<State>,
}

fn class_registered() -> Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.load(Ordering::SeqCst) {
        return Ok(());
    }
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_DROPSHADOW,
        lpfnWndProc: Some(wndproc),
        hInstance: crate::dll::hinstance(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: w!("KhmerAutocompleteCandidates"),
        ..Default::default()
    };
    // a second registration (another thread of the same process) fails with "class exists": that is fine
    unsafe { RegisterClassExW(&class) };
    DONE.store(true, Ordering::SeqCst);
    Ok(())
}

impl CandidateWindow {
    pub fn new(on_pick: Box<dyn Fn(usize)>) -> Result<CandidateWindow> {
        class_registered()?;
        let d2d: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                w!("KhmerAutocompleteCandidates"),
                w!(""),
                WS_POPUP,
                0,
                0,
                10,
                10,
                None,
                None,
                Some(crate::dll::hinstance()),
                None,
            )?
        };
        let state = Box::new(State {
            on_pick,
            d2d,
            dwrite,
            format: RefCell::new(None),
            items: RefCell::new(Vec::new()),
            is_label: Cell::new(false),
            target: RefCell::new(None),
            px_size: Cell::new((0, 0)),
            scale: Cell::new(1.0),
        });
        // the Box never moves, so the window procedure can find the state through this pointer
        // (isize on 64-bit Windows, i32 on 32-bit: `as _` follows the target)
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&*state as *const State as usize) as _) };
        Ok(CandidateWindow { hwnd, state })
    }

    /// Show candidates (already formatted) below `anchor`, the caret rectangle in screen coordinates.
    pub fn show(&self, items: Vec<String>, anchor: Option<RECT>) {
        self.state.is_label.set(false);
        let _ = unsafe { KillTimer(Some(self.hwnd), LABEL_TIMER) };
        self.place(items, anchor);
    }

    /// Show a short message (the new input mode) that disappears by itself.
    pub fn show_label(&self, text: &str, anchor: Option<RECT>) {
        self.state.is_label.set(true);
        self.place(vec![text.to_string()], anchor);
        unsafe { SetTimer(Some(self.hwnd), LABEL_TIMER, LABEL_MS, None) };
    }

    pub fn hide(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), LABEL_TIMER) };
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn place(&self, texts: Vec<String>, anchor: Option<RECT>) {
        let s = &*self.state;
        let dpi = unsafe { GetDpiForWindow(self.hwnd) }.max(96);
        let scale = dpi as f32 / 96.0;
        s.scale.set(scale);
        // the text format follows the window's DPI
        let needs_format = s.format.borrow().as_ref().is_none_or(|(_, d)| *d != dpi);
        if needs_format {
            let f = unsafe {
                s.dwrite.CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    FONT_PX * scale,
                    w!("en-us"),
                )
            };
            match f {
                Ok(f) => {
                    let _ = unsafe { f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP) };
                    *s.format.borrow_mut() = Some((f, dpi));
                }
                Err(_) => return,
            }
        }
        let format = s.format.borrow().as_ref().map(|(f, _)| f.clone()).expect("format");
        let (mut items, mut x, mut height) = (Vec::new(), PAD_X * scale / 2.0, 0f32);
        for t in &texts {
            let wide: Vec<u16> = t.encode_utf16().collect();
            let Ok(layout) = (unsafe { s.dwrite.CreateTextLayout(&wide, &format, 2000.0, 200.0) }) else { return };
            let mut m = DWRITE_TEXT_METRICS::default();
            if unsafe { layout.GetMetrics(&mut m) }.is_err() {
                return;
            }
            let w = m.width + PAD_X * scale * 2.0;
            height = height.max(m.height);
            items.push(Item { layout, x, w });
            x += w + GAP * scale;
        }
        let total_w = (x - GAP * scale + PAD_X * scale / 2.0).ceil().max(20.0) as i32;
        let total_h = (height + PAD_Y * scale * 2.0).ceil() as i32;
        *s.items.borrow_mut() = items;

        // below the caret, or above it when there is no room; always on the monitor the caret is on
        let (mut px, mut py) = anchor.map_or((100, 100), |r| (r.left, r.bottom + 2));
        unsafe {
            let mon = MonitorFromPoint(POINT { x: px, y: py }, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if GetMonitorInfoW(mon, &mut mi).as_bool() {
                let work = mi.rcWork;
                if py + total_h > work.bottom {
                    py = anchor.map_or(py, |r| r.top - total_h - 2);
                }
                px = px.min(work.right - total_w).max(work.left);
                py = py.max(work.top);
            }
            let _ =
                SetWindowPos(self.hwnd, Some(HWND_TOPMOST), px, py, total_w, total_h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }
}

impl Drop for CandidateWindow {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn colorref(c: u32) -> D2D1_COLOR_F {
    let c = COLORREF(c).0;
    D2D1_COLOR_F {
        r: (c & 0xFF) as f32 / 255.0,
        g: ((c >> 8) & 0xFF) as f32 / 255.0,
        b: ((c >> 16) & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

impl State {
    fn paint(&self, hwnd: HWND) -> Result<()> {
        let mut rc = RECT::default();
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rc)? };
        let size = ((rc.right - rc.left).max(1) as u32, (rc.bottom - rc.top).max(1) as u32);
        if self.target.borrow().is_none() {
            let rt = unsafe {
                self.d2d.CreateHwndRenderTarget(
                    // 96 dpi: we work in pixels and scale fonts ourselves
                    &D2D1_RENDER_TARGET_PROPERTIES { dpiX: 96.0, dpiY: 96.0, ..Default::default() },
                    &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                        hwnd,
                        pixelSize: D2D_SIZE_U { width: size.0, height: size.1 },
                        presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                    },
                )?
            };
            *self.target.borrow_mut() = Some(rt);
            self.px_size.set(size);
        }
        let target = self.target.borrow().clone().expect("target");
        if self.px_size.get() != size {
            unsafe { target.Resize(&D2D_SIZE_U { width: size.0, height: size.1 })? };
            self.px_size.set(size);
        }
        unsafe {
            let (bg, fg, sel_bg, sel_fg, dim) = (
                colorref(GetSysColor(COLOR_WINDOW)),
                colorref(GetSysColor(COLOR_WINDOWTEXT)),
                colorref(GetSysColor(COLOR_HIGHLIGHT)),
                colorref(GetSysColor(COLOR_HIGHLIGHTTEXT)),
                colorref(GetSysColor(COLOR_GRAYTEXT)),
            );
            target.BeginDraw();
            target.Clear(Some(&bg));
            let scale = self.scale.get();
            let (w, h) = (size.0 as f32, size.1 as f32);
            for (i, item) in self.items.borrow().iter().enumerate() {
                // the first candidate is the one Tab takes; a label has no selection
                let first = i == 0 && !self.is_label.get();
                if first {
                    let brush = target.CreateSolidColorBrush(&sel_bg, None)?;
                    target.FillRectangle(
                        &D2D_RECT_F { left: item.x, top: 2.0, right: item.x + item.w, bottom: h - 2.0 },
                        &brush,
                    );
                }
                let brush = target.CreateSolidColorBrush(
                    if first {
                        &sel_fg
                    } else if self.is_label.get() {
                        &dim
                    } else {
                        &fg
                    },
                    None,
                )?;
                target.DrawTextLayout(
                    Vector2 { X: item.x + PAD_X * scale, Y: PAD_Y * scale },
                    &item.layout,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
            let border = target.CreateSolidColorBrush(&dim, None)?;
            target.DrawRectangle(
                &D2D_RECT_F { left: 0.5, top: 0.5, right: w - 0.5, bottom: h - 0.5 },
                &border,
                1.0,
                None,
            );
            if target.EndDraw(None, None).is_err() {
                // the device was lost: build a new target on the next paint
                *self.target.borrow_mut() = None;
            }
        }
        Ok(())
    }

    fn click(&self, x: i32) {
        if self.is_label.get() {
            return;
        }
        let hit = self.items.borrow().iter().position(|it| (x as f32) >= it.x && (x as f32) < it.x + it.w);
        if let Some(i) = hit {
            (self.on_pick)(i);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let state = unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as usize as *const State).as_ref() };
    // nothing may unwind out of a window procedure
    let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match (msg, state) {
        (WM_MOUSEACTIVATE, _) => Some(LRESULT(MA_NOACTIVATE as isize)),
        (WM_ERASEBKGND, _) => Some(LRESULT(1)),
        (WM_PAINT, Some(s)) => {
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(hwnd, &mut ps) };
            let _ = s.paint(hwnd);
            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            Some(LRESULT(0))
        }
        (WM_LBUTTONDOWN, Some(s)) => {
            s.click((lparam.0 & 0xFFFF) as i16 as i32);
            Some(LRESULT(0))
        }
        (WM_TIMER, _) if wparam.0 == LABEL_TIMER => unsafe {
            let _ = KillTimer(Some(hwnd), LABEL_TIMER);
            let _ = ShowWindow(hwnd, SW_HIDE);
            Some(LRESULT(0))
        },
        _ => None,
    }));
    match handled {
        Ok(Some(r)) => r,
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
