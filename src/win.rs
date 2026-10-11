//! Dünne Schicht über die Win32-API: Fenster auflisten, Position/Größe lesen und setzen.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::OnceLock;
use std::sync::mpsc;

use windows::Win32::Foundation::{
    CloseHandle, E_ACCESSDENIED, HANDLE, HWND, LPARAM, POINT, RECT, WPARAM,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWINDOWATTRIBUTE, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GdiFlush, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST,
    MONITORINFO, MonitorFromRect, MonitorFromWindow, SelectObject,
};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    DI_NORMAL, DrawIconEx, EVENT_OBJECT_LOCATIONCHANGE, EnumWindows, GCLP_HICON, GCLP_HICONSM, GWL_EXSTYLE, GetClassLongPtrW,
    GetClassNameW, GetCursorPos, GetWindowLongW, GetWindowPlacement, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, HICON, ICON_BIG, ICON_SMALL2, IsIconic, IsWindow,
    IsWindowVisible, IsZoomed, MB_ICONERROR, MessageBoxW, OBJID_WINDOW, SMTO_ABORTIFHUNG, SW_RESTORE,
    SWP_NOACTIVATE, SWP_NOZORDER, SendMessageTimeoutW, SetCursorPos, SetWindowPos, ShowWindow,
    WINDOWPLACEMENT, WINEVENT_OUTOFCONTEXT, WM_GETICON, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, HSTRING};

#[derive(Clone)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    /// Das Fenster läuft mit höheren Rechten als diese App und lässt sich nicht ändern.
    pub elevated: bool,
}

/// Rechteck in physischen Bildschirmpixeln.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl From<RECT> for Rect {
    fn from(r: RECT) -> Self {
        Self {
            x: r.left,
            y: r.top,
            w: r.right - r.left,
            h: r.bottom - r.top,
        }
    }
}

/// Alle sichtbaren Hauptfenster anderer Prozesse, in Z-Reihenfolge (vorderstes zuerst).
pub fn list_windows() -> Vec<WindowInfo> {
    let mut windows: Vec<WindowInfo> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
        );
    }
    windows
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<WindowInfo>) };
    if let Some(title) = unsafe { resizable_window_title(hwnd) } {
        let elevated = unsafe { is_elevated(hwnd) };
        windows.push(WindowInfo {
            hwnd,
            title,
            elevated,
        });
    }
    BOOL::from(true)
}

unsafe fn resizable_window_title(hwnd: HWND) -> Option<String> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return None;
        }

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }

        // Fenster auf anderen virtuellen Desktops und angehaltene UWP-Apps sind "cloaked".
        let mut cloaked = 0u32;
        if dwm_attribute(hwnd, DWMWA_CLOAKED, &mut cloaked) && cloaked != 0 {
            return None;
        }

        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class) as usize;
        if String::from_utf16_lossy(&class[..len]) == "Progman" {
            return None;
        }

        let mut title = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title) as usize;
        (len > 0).then(|| String::from_utf16_lossy(&title[..len]))
    }
}

/// Integritätsstufe, ab der ein Prozess mit Administratorrechten läuft.
const HIGH_INTEGRITY: u32 = 0x3000;

/// Ob der Prozess des Fensters eine höhere Integritätsstufe hat als diese App. Windows
/// verweigert dann das Verschieben (UIPI). Läuft die App selbst als Administrator, gilt
/// das für kein gewöhnliches Fenster mehr.
unsafe fn is_elevated(hwnd: HWND) -> bool {
    unsafe {
        static OWN: OnceLock<u32> = OnceLock::new();
        let own = *OWN.get_or_init(|| integrity_level(GetCurrentProcess()).unwrap_or(0));
        // Lässt sich die Stufe nicht lesen, liegt das meist selbst an fehlenden Rechten.
        let unknown = own < HIGH_INTEGRITY;

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return unknown;
        };
        let level = integrity_level(process);
        let _ = CloseHandle(process);
        level.map_or(unknown, |level| level > own)
    }
}

unsafe fn integrity_level(process: HANDLE) -> Option<u32> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        // Auf die Struktur folgt die SID, auf die sie zeigt. `u64` sorgt für die Ausrichtung.
        let mut buffer = [0u64; 16];
        let mut len = 0u32;
        let result = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr() as *mut c_void),
            size_of_val(&buffer) as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        result.ok()?;

        let sid = (*(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL)).Label.Sid;
        let count = *GetSidSubAuthorityCount(sid);
        Some(*GetSidSubAuthority(sid, count.checked_sub(1)? as u32))
    }
}

unsafe fn dwm_attribute<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &mut T) -> bool {
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            attribute,
            value as *mut T as *mut c_void,
            size_of::<T>() as u32,
        )
        .is_ok()
    }
}

/// Kantenlänge der Fenstersymbole in Pixeln.
pub const ICON_SIZE: usize = 32;

/// Symbol des Fensters mit `ICON_SIZE` Kantenlänge als RGBA mit vormultipliziertem Alpha.
pub fn icon(hwnd: HWND) -> Option<Vec<u8>> {
    unsafe {
        let icon = icon_handle(hwnd)?;
        let size = ICON_SIZE as i32;
        let len = ICON_SIZE * ICON_SIZE * 4;
        // Doppelte Höhe: Das Symbol wird einmal auf Schwarz und einmal auf Weiß gezeichnet.
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -2 * size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let dc = CreateCompatibleDC(None);
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            return None;
        };
        let previous = SelectObject(dc, bitmap.into());
        let (black, white) =
            std::slice::from_raw_parts_mut(bits as *mut u8, 2 * len).split_at_mut(len);
        white.fill(0xFF);
        let drawn = [0, size]
            .into_iter()
            .all(|y| DrawIconEx(dc, 0, y, icon, size, size, 0, None, DI_NORMAL).is_ok());
        let _ = GdiFlush();

        // Ältere Symbole haben keinen Alphakanal, nur eine Maske. Aus dem Unterschied
        // zwischen beiden Hintergründen ergibt sich die Deckkraft in jedem Fall.
        let rgba: Vec<u8> = black
            .as_chunks::<4>()
            .0
            .iter()
            .zip(white.as_chunks::<4>().0)
            .flat_map(|(b, w)| [b[2], b[1], b[0], 255 - w[1].saturating_sub(b[1])])
            .collect();

        SelectObject(dc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);

        let visible = rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
        (drawn && visible).then_some(rgba)
    }
}

/// Das Symbol gehört dem Fenster bzw. seiner Klasse und darf nicht freigegeben werden.
unsafe fn icon_handle(hwnd: HWND) -> Option<HICON> {
    unsafe {
        // Mit Zeitlimit: Ein hängendes Fenster würde sonst die Oberfläche blockieren.
        let from_window = |kind: u32| {
            let mut handle = 0usize;
            SendMessageTimeoutW(
                hwnd,
                WM_GETICON,
                WPARAM(kind as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                50,
                Some(&mut handle),
            );
            Some(handle).filter(|&handle| handle != 0)
        };
        let from_class = |index| Some(GetClassLongPtrW(hwnd, index)).filter(|&handle| handle != 0);

        from_window(ICON_BIG)
            .or_else(|| from_class(GCLP_HICON))
            .or_else(|| from_window(ICON_SMALL2))
            .or_else(|| from_class(GCLP_HICONSM))
            .map(|handle| HICON(handle as *mut c_void))
    }
}

/// Primärsprache der Windows-Oberfläche (`LANG_*`), etwa `0x07` für Deutsch.
pub fn ui_language() -> u16 {
    unsafe { GetUserDefaultUILanguage() & 0x3FF }
}

/// Fehlermeldung für den Fall, dass die App kein eigenes Fenster hat, um sie zu zeigen.
pub fn error_box(title: &str, text: &str) {
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), &HSTRING::from(title), MB_ICONERROR);
    }
}

pub fn exists(hwnd: HWND) -> bool {
    unsafe { IsWindow(Some(hwnd)).as_bool() }
}

/// Sichtbarer Fensterrahmen, ohne die unsichtbaren Resize-Ränder von Windows 10/11.
/// Für minimierte Fenster die Position, an der sie wiederhergestellt würden.
pub fn rect(hwnd: HWND) -> Option<Rect> {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let mut placement = WINDOWPLACEMENT {
                length: size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            GetWindowPlacement(hwnd, &mut placement).ok()?;
            // Die Angabe zählt ab dem Arbeitsbereich, nicht ab der Monitorecke: Mit der
            // Taskleiste oben oder links läge sie um deren Breite daneben.
            let normal = placement.rcNormalPosition;
            let monitor = MonitorFromRect(&normal, MONITOR_DEFAULTTONEAREST);
            let (dx, dy) = monitor_info(monitor).map_or((0, 0), |info| {
                (
                    info.rcWork.left - info.rcMonitor.left,
                    info.rcWork.top - info.rcMonitor.top,
                )
            });
            let normal = Rect::from(normal);
            return Some(Rect {
                x: normal.x + dx,
                y: normal.y + dy,
                ..normal
            });
        }
        frame_bounds(hwnd).map(Rect::from)
    }
}

unsafe fn frame_bounds(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let mut bounds = RECT::default();
        if dwm_attribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut bounds) {
            return Some(bounds);
        }
        GetWindowRect(hwnd, &mut bounds).ok()?;
        Some(bounds)
    }
}

/// Arbeitsbereich (ohne Taskleiste) des Monitors, auf dem das Fenster liegt.
pub fn work_area(hwnd: HWND) -> Option<Rect> {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        monitor_info(monitor).map(|info| info.rcWork.into())
    }
}

unsafe fn monitor_info(monitor: HMONITOR) -> Option<MONITORINFO> {
    unsafe {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(monitor, &mut info).as_bool().then_some(info)
    }
}

struct Watch {
    hook: HWINEVENTHOOK,
    hwnd: HWND,
    notify: Box<dyn Fn()>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWinEvent(self.hook);
        }
    }
}

thread_local! {
    static WATCH: RefCell<Option<Watch>> = const { RefCell::new(None) };
}

/// Ruft `notify` auf, sobald sich Position oder Größe von `hwnd` ändert, und löst eine
/// frühere Beobachtung ab. Windows stellt die Meldungen über die Nachrichtenschleife des
/// aufrufenden Threads zu.
pub fn watch(hwnd: HWND, notify: impl Fn() + 'static) {
    unwatch();
    unsafe {
        let mut pid = 0u32;
        let thread = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let hook = SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(location_changed),
            pid,
            thread,
            WINEVENT_OUTOFCONTEXT,
        );
        if hook.is_invalid() {
            return;
        }
        let watch = Watch {
            hook,
            hwnd,
            notify: Box::new(notify),
        };
        WATCH.with(|slot| *slot.borrow_mut() = Some(watch));
    }
}

pub fn unwatch() {
    WATCH.with(|slot| slot.borrow_mut().take());
}

unsafe extern "system" fn location_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    // Das Ereignis kommt auch für Kindfenster, den Mauszeiger und die Schreibmarke.
    if object != OBJID_WINDOW.0 {
        return;
    }
    WATCH.with(|slot| {
        if let Ok(slot) = slot.try_borrow()
            && let Some(watch) = slot.as_ref()
            && watch.hwnd == hwnd
        {
            (watch.notify)();
        }
    });
}

/// Position des Mauszeigers in physischen Bildschirmpixeln.
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x, point.y))
}

pub fn set_cursor_pos((x, y): (i32, i32)) {
    unsafe {
        let _ = SetCursorPos(x, y);
    }
}

pub enum SetError {
    AccessDenied,
    Other(String),
}

/// Setzt die Lage eines Fensters in einem eigenen Thread. Windows wartet beim Setzen auf
/// das fremde Fenster: Reagiert es träge oder gar nicht, stünde sonst die Oberfläche still.
pub struct Mover {
    requests: mpsc::Sender<(u64, Rect)>,
    results: mpsc::Receiver<(u64, Rect, Result<(), SetError>)>,
    sent: u64,
    done: u64,
}

impl Mover {
    /// `notify` läuft im Thread des `Mover`, sobald ein Ergebnis für `poll` bereitliegt.
    pub fn new(hwnd: HWND, notify: impl Fn() + Send + 'static) -> Self {
        let (requests, queue) = mpsc::channel::<(u64, Rect)>();
        let (finished, results) = mpsc::channel();
        let hwnd = hwnd.0 as isize;
        // Endet, sobald der `Mover` und damit der Sender wegfällt.
        std::thread::spawn(move || {
            while let Ok(mut request) = queue.recv() {
                // Nur die jüngste Lage zählt, ein träges Fenster überspringt die älteren.
                while let Ok(newer) = queue.try_recv() {
                    request = newer;
                }
                let (id, target) = request;
                let result = set_rect(HWND(hwnd as *mut c_void), target).map_err(|err| {
                    if err.code() == E_ACCESSDENIED {
                        SetError::AccessDenied
                    } else {
                        SetError::Other(err.message())
                    }
                });
                if finished.send((id, target, result)).is_err() {
                    break;
                }
                notify();
            }
        });
        Self {
            requests,
            results,
            sent: 0,
            done: 0,
        }
    }

    pub fn request(&mut self, target: Rect) {
        self.sent += 1;
        let _ = self.requests.send((self.sent, target));
    }

    /// Zuletzt gesetzte Lage und ihr Ergebnis, falls seit dem letzten Aufruf eine fertig wurde.
    pub fn poll(&mut self) -> Option<(Rect, Result<(), SetError>)> {
        let (id, target, result) = self.results.try_iter().last()?;
        self.done = id;
        Some((target, result))
    }

    /// Ob noch eine angeforderte Lage aussteht.
    pub fn busy(&self) -> bool {
        self.done != self.sent
    }
}

/// Setzt den sichtbaren Fensterrahmen auf `target`. Maximierte und minimierte Fenster
/// werden vorher wiederhergestellt, sonst würde Windows die neue Größe ignorieren.
fn set_rect(hwnd: HWND, target: Rect) -> windows::core::Result<()> {
    unsafe {
        if IsIconic(hwnd).as_bool() || IsZoomed(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        // GetWindowRect enthält die unsichtbaren Ränder, DWM nicht. Die Differenz wird
        // aufgeschlagen, damit der sichtbare Rahmen exakt auf den Zielwerten landet.
        let mut outer = RECT::default();
        GetWindowRect(hwnd, &mut outer)?;
        let visible = frame_bounds(hwnd).unwrap_or(outer);
        let inset = |outer: i32, visible: i32| {
            let d = (visible - outer).abs();
            if d <= 32 { d } else { 0 }
        };
        let left = inset(outer.left, visible.left);
        let top = inset(outer.top, visible.top);
        let right = inset(outer.right, visible.right);
        let bottom = inset(outer.bottom, visible.bottom);

        SetWindowPos(
            hwnd,
            None,
            target.x - left,
            target.y - top,
            target.w + left + right,
            target.h + top + bottom,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
}
