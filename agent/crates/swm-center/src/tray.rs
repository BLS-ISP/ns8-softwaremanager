use std::sync::{Arc, RwLock};
use swm_common::{CenterStatusSnapshot, RebootPolicy, RebootState};
use tokio::sync::mpsc::UnboundedSender;

pub const POWERSHELL_TOAST_APP_ID: &str =
    r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayMenuAction {
    OpenSoftwareCenter,
    CheckForUpdates,
    ShowStatus,
    SnoozeRebootMinutes(u32),
    RebootNow,
    QuitTray,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopNotification {
    pub title: String,
    pub body: String,
    pub urgent: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TrayStateTracker {
    pub minimized_to_tray: bool,
    last_notified_reboot: bool,
    last_final_warning_sent: bool,
    last_seen_job_id: Option<String>,
}

impl TrayStateTracker {
    pub fn new(minimized_to_tray: bool) -> Self {
        Self {
            minimized_to_tray,
            last_notified_reboot: false,
            last_final_warning_sent: false,
            last_seen_job_id: None,
        }
    }

    pub fn on_window_close_requested(&mut self) -> bool {
        // Minimize to system tray instead of terminating process
        self.minimized_to_tray = true;
        true
    }

    pub fn restore_window(&mut self) {
        self.minimized_to_tray = false;
    }

    pub fn can_snooze_reboot(reboot_state: &RebootState, now_unix: i64) -> bool {
        reboot_state.reboot_required
            && reboot_state.policy == RebootPolicy::GracePeriod
            && reboot_state
                .deadline_unix
                .map(|dl| now_unix < dl)
                .unwrap_or(true)
    }

    pub fn available_menu_items(reboot_state: &RebootState, now_unix: i64) -> Vec<TrayMenuAction> {
        let mut items = vec![
            TrayMenuAction::OpenSoftwareCenter,
            TrayMenuAction::CheckForUpdates,
            TrayMenuAction::ShowStatus,
        ];
        if reboot_state.reboot_required {
            if Self::can_snooze_reboot(reboot_state, now_unix) {
                items.push(TrayMenuAction::SnoozeRebootMinutes(60));
                items.push(TrayMenuAction::SnoozeRebootMinutes(240));
            }
            items.push(TrayMenuAction::RebootNow);
        }
        items.push(TrayMenuAction::QuitTray);
        items
    }

    /// Evaluates an incoming `CenterStatusSnapshot` and returns any desktop notification
    /// that should be surfaced to the logged-in user (respecting active snooze timers).
    pub fn evaluate_notifications(
        &mut self,
        status: &CenterStatusSnapshot,
        now_unix: i64,
    ) -> Vec<DesktopNotification> {
        let mut notes = Vec::new();

        // 1. Notify on newly started mandatory background jobs
        if let Some(active) = status.active_jobs.first() {
            if self.last_seen_job_id.as_deref() != Some(active.job_id.as_str()) {
                self.last_seen_job_id = Some(active.job_id.clone());
                notes.push(DesktopNotification {
                    title: "Software Center – Installation aktiv".to_string(),
                    body: format!("{} ({}) wird installiert…", active.target_name, active.version),
                    urgent: false,
                });
            }
        }

        // 2. Notify on pending reboot & final countdown (< 15 min before deadline)
        let rb = &status.reboot_state;
        if !rb.reboot_required {
            self.last_notified_reboot = false;
            self.last_final_warning_sent = false;
            return notes;
        }

        let is_snoozed = rb
            .snoozed_until_unix
            .map(|until| now_unix < until)
            .unwrap_or(false);

        if let Some(deadline) = rb.deadline_unix {
            let remaining_secs = deadline - now_unix;
            if remaining_secs <= 900 && !self.last_final_warning_sent {
                self.last_final_warning_sent = true;
                let mins = (remaining_secs.max(0) + 59) / 60;
                notes.push(DesktopNotification {
                    title: "Dringend: Systemneustart erforderlich".to_string(),
                    body: format!(
                        "Bitte speichern Sie Ihre Arbeit. Automatischer Neustart in {mins} Minute(n)."
                    ),
                    urgent: true,
                });
                return notes;
            }
        }

        if !is_snoozed && !self.last_notified_reboot {
            self.last_notified_reboot = true;
            notes.push(DesktopNotification {
                title: "Neustart erforderlich".to_string(),
                body: "Ein installiertes Update erfordert einen Neustart Ihres Computers."
                    .to_string(),
                urgent: false,
            });
        }

        notes
    }
}

/// Escapes special XML characters so arbitrary package titles or messages are safe inside WinRT Toast XML.
pub fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Builds a WinRT `ToastGeneric` XML document payload for Windows 10/11 Action Center.
pub fn build_winrt_toast_xml(note: &DesktopNotification) -> String {
    let duration = if note.urgent { "long" } else { "short" };
    let title = escape_xml(&note.title);
    let body = escape_xml(&note.body);
    format!(
        "<toast duration=\"{duration}\"><visual><binding template=\"ToastGeneric\"><text>{title}</text><text>{body}</text></binding></visual></toast>"
    )
}

/// Builds the PowerShell WinRT toast command invoking `ToastNotificationManager` + `XmlDocument`.
pub fn build_powershell_toast_script(note: &DesktopNotification) -> String {
    let toast_xml = build_winrt_toast_xml(note).replace('\'', "''");
    let app_id = POWERSHELL_TOAST_APP_ID.replace('\'', "''");
    format!(
        "$ErrorActionPreference = 'SilentlyContinue'; \
         [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
         [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] > $null; \
         $xml = New-Object Windows.Data.Xml.Dom.XmlDocument; \
         $xml.LoadXml('{toast_xml}'); \
         $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{app_id}').Show($toast)"
    )
}

/// Determines whether interactive Win32 tray handles should be created.
/// Skipped when `--no-open` is passed or `SWM_HEADLESS_TRAY=1` is set so CI/headless tests never block.
pub fn should_spawn_native_tray(no_open: bool) -> bool {
    if no_open {
        return false;
    }
    if std::env::var("SWM_HEADLESS_TRAY").ok().as_deref() == Some("1") {
        return false;
    }
    true
}

/// Enabled by default in interactive `gui` mode unless `SWM_DISABLE_DESKTOP_NOTIFY=1` or `SWM_HEADLESS_TRAY=1` is set.
pub fn notifications_enabled() -> bool {
    if std::env::var("SWM_DISABLE_DESKTOP_NOTIFY")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false)
    {
        return false;
    }
    if std::env::var("SWM_HEADLESS_TRAY").ok().as_deref() == Some("1") {
        return false;
    }
    if std::env::var("SWM_ENABLE_DESKTOP_NOTIFY").ok().as_deref() == Some("0") {
        return false;
    }
    true
}

fn dist_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let vx = bx - ax;
    let vy = by - ay;
    let wx = px - ax;
    let wy = py - ay;
    let len_sq = vx * vx + vy * vy;
    let t = if len_sq > 0.0 {
        ((wx * vx + wy * vy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dx = px - (ax + t * vx);
    let dy = py - (ay + t * vy);
    (dx * dx + dy * dy).sqrt()
}

/// Procedurally renders a 32x32 premultiplied-BGRA cyan/indigo shield-cube icon (`[B, G, R, A]`).
/// Zero external `.ico` file dependency and crisp DPI scaling on Windows 10/11 system trays.
pub fn render_shield_cube_bgra32() -> Vec<u8> {
    const W: usize = 32;
    const H: usize = 32;
    let mut buf = vec![0u8; W * H * 4];

    for y in 0..H {
        for x in 0..W {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = (fx - 16.0).abs();

            // Shield silhouette: broad rounded shoulders at top, tapering to point at y=29.5
            let half_width = if fy < 4.5 {
                let arch = ((fy - 2.0) / 2.5).clamp(0.0, 1.0);
                4.5 + 8.0 * arch.sqrt()
            } else if fy <= 15.5 {
                12.5
            } else if fy <= 29.8 {
                let t = ((fy - 15.5) / 14.3).clamp(0.0, 1.0);
                12.5 * (1.0 - t.powf(1.28))
            } else {
                0.0
            };

            let vertical_inside = fy >= 2.0 && fy <= 29.8;
            let signed_edge = half_width - dx;
            let shield_alpha = if !vertical_inside {
                0.0
            } else {
                (signed_edge + 0.5).clamp(0.0, 1.0)
            };

            if shield_alpha <= 0.0 {
                continue;
            }

            // Cyan (#06b6d4) -> Royal Blue (#2563eb) -> Indigo (#4f46e5) diagonal gradient
            let grad_t = ((fx * 0.45 + fy * 0.75) / 32.0).clamp(0.0, 1.0);
            let (mut r, mut g, mut b) = if grad_t < 0.55 {
                let k = grad_t / 0.55;
                (
                    6.0 + (37.0 - 6.0) * k,
                    182.0 + (99.0 - 182.0) * k,
                    212.0 + (235.0 - 212.0) * k,
                )
            } else {
                let k = (grad_t - 0.55) / 0.45;
                (
                    37.0 + (79.0 - 37.0) * k,
                    99.0 + (70.0 - 99.0) * k,
                    235.0 + (229.0 - 235.0) * k,
                )
            };

            // Subtle bright rim highlight along shield border
            if signed_edge < 1.35 {
                let rim = (1.0 - (signed_edge / 1.35).clamp(0.0, 1.0)) * 0.28;
                r = r + (255.0 - r) * rim;
                g = g + (255.0 - g) * rim;
                b = b + (255.0 - b) * rim;
            }

            // Isometric 3D package cube centered at (16.0, 15.2), radius R = 6.6
            let cx = 16.0f32;
            let cy = 15.2f32;
            let rad = 6.6f32;
            let rx = rad * 0.866_025_4; // cos(30°)
            let ry = rad * 0.5; // sin(30°)

            let top = (cx, cy - rad);
            let tr = (cx + rx, cy - ry);
            let br = (cx + rx, cy + ry);
            let bot = (cx, cy + rad);
            let bl = (cx - rx, cy + ry);
            let tl = (cx - rx, cy - ry);
            let mid = (cx, cy + 0.3);

            let segments = [
                // Outer hexagon
                (top.0, top.1, tr.0, tr.1),
                (tr.0, tr.1, br.0, br.1),
                (br.0, br.1, bot.0, bot.1),
                (bot.0, bot.1, bl.0, bl.1),
                (bl.0, bl.1, tl.0, tl.1),
                (tl.0, tl.1, top.0, top.1),
                // Inner Y cube seams
                (tl.0, tl.1, mid.0, mid.1),
                (tr.0, tr.1, mid.0, mid.1),
                (bot.0, bot.1, mid.0, mid.1),
            ];

            let mut min_seg_dist = f32::MAX;
            for &(ax, ay, bx, by) in &segments {
                let d = dist_to_segment(fx, fy, ax, ay, bx, by);
                if d < min_seg_dist {
                    min_seg_dist = d;
                }
            }

            // Soft translucent top-lid tint inside cube for depth
            let in_top_diamond = {
                let u = (fx - cx).abs() / rx + (fy - (cy - ry)).abs() / ry;
                u <= 0.98
            };
            if in_top_diamond {
                r = r + (255.0 - r) * 0.22;
                g = g + (255.0 - g) * 0.26;
                b = b + (255.0 - b) * 0.26;
            }

            // Anti-aliased crisp white cube wireframe strokes (stroke radius ~0.88px)
            let line_cov = (1.25 - min_seg_dist).clamp(0.0, 1.0);
            if line_cov > 0.0 {
                r = r + (255.0 - r) * line_cov;
                g = g + (255.0 - g) * line_cov;
                b = b + (255.0 - b) * line_cov;
            }

            // Premultiplied BGRA output
            let a_u8 = (shield_alpha * 255.0).round() as u8;
            let b_u8 = ((b * shield_alpha).round().clamp(0.0, 255.0)) as u8;
            let g_u8 = ((g * shield_alpha).round().clamp(0.0, 255.0)) as u8;
            let r_u8 = ((r * shield_alpha).round().clamp(0.0, 255.0)) as u8;

            let idx = (y * W + x) * 4;
            buf[idx] = b_u8;
            buf[idx + 1] = g_u8;
            buf[idx + 2] = r_u8;
            buf[idx + 3] = a_u8;
        }
    }

    buf
}

pub fn dispatch_os_notification(note: &DesktopNotification) {
    if !notifications_enabled() {
        tracing::info!(
            urgent = note.urgent,
            "[Tray Notification (suppressed)] {}: {}",
            note.title,
            note.body
        );
        return;
    }

    tracing::info!(
        urgent = note.urgent,
        "[Tray Notification] {}: {}",
        note.title,
        note.body
    );

    #[cfg(unix)]
    {
        let urgency = if note.urgent { "critical" } else { "normal" };
        let _ = std::process::Command::new("notify-send")
            .args([
                "-a",
                "NS8 Software Center",
                "-u",
                urgency,
                &note.title,
                &note.body,
            ])
            .spawn();
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let ps_script = build_powershell_toast_script(note);
        let res = std::process::Command::new("powershell.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &ps_script,
            ])
            .spawn();

        if let Err(err) = res {
            tracing::warn!("Failed spawning WinRT PowerShell toast notification: {err}");
        }
    }
}

pub fn launch_desktop_window(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        // Prefer Microsoft Edge WebView/App-mode window (native frameless look without address bar)
        if std::process::Command::new("cmd.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/C", "start", "", "msedge", &format!("--app={url}")])
            .spawn()
            .is_err()
        {
            let _ = std::process::Command::new("cmd.exe")
                .creation_flags(CREATE_NO_WINDOW)
                .args(["/C", "start", "", url])
                .spawn();
        }
    }

    #[cfg(unix)]
    {
        if std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok() {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    }
}

#[cfg(windows)]
mod win32_tray {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_WARNING,
        NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
        PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
        TranslateMessage, CW_USEDEFAULT, HICON, ICONINFO, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG,
        TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_CONTEXTMENU,
        WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WM_USER, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    };

    const WM_TRAYICON: u32 = WM_USER + 1;
    const WM_TRAY_SHUTDOWN: u32 = WM_USER + 2;
    const WM_TRAY_BALLOON: u32 = WM_USER + 3;

    const IDM_OPEN_CENTER: usize = 1001;
    const IDM_CHECK_UPDATES: usize = 1002;
    const IDM_SNOOZE_1H: usize = 1003;
    const IDM_SNOOZE_4H: usize = 1004;
    const IDM_REBOOT_NOW: usize = 1005;
    const IDM_QUIT: usize = 1006;

    struct Win32TrayThreadState {
        nid: NOTIFYICONDATAW,
        hicon: HICON,
        center_url: String,
        shared_reboot: Arc<RwLock<RebootState>>,
        action_tx: UnboundedSender<TrayMenuAction>,
        balloon_queue: Arc<Mutex<VecDeque<DesktopNotification>>>,
        icon_added: bool,
    }

    thread_local! {
        static TRAY_STATE: RefCell<Option<Win32TrayThreadState>> = const { RefCell::new(None) };
    }

    fn to_wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn copy_wide_fixed<const N: usize>(dst: &mut [u16; N], src: &str) {
        dst.fill(0);
        for (i, unit) in src.encode_utf16().take(N.saturating_sub(1)).enumerate() {
            dst[i] = unit;
        }
    }

    unsafe fn create_shield_cube_hicon() -> HICON {
        let pixels = render_shield_cube_bgra32();
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32, // top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: pixels.len() as u32,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [std::mem::zeroed()],
        };

        let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm_color = CreateDIBSection(
            0,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            0,
            0,
        );
        if hbm_color == 0 || bits_ptr.is_null() {
            return 0;
        }

        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits_ptr as *mut u8, pixels.len());

        // 32x32 1-bpp mask (all 0s; per-pixel alpha channel in 32-bpp DIB controls transparency)
        let mask_bytes = [0u8; 128];
        let hbm_mask = CreateBitmap(32, 32, 1, 1, mask_bytes.as_ptr() as *const _);
        if hbm_mask == 0 {
            DeleteObject(hbm_color as _);
            return 0;
        }

        let icon_info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let hicon = CreateIconIndirect(&icon_info);

        DeleteObject(hbm_mask as _);
        DeleteObject(hbm_color as _);
        hicon
    }

    unsafe fn show_context_menu(hwnd: HWND, st: &Win32TrayThreadState) {
        let hmenu = CreatePopupMenu();
        if hmenu == 0 {
            return;
        }

        let open_lbl = to_wide_null("Software Center öffnen");
        let updates_lbl = to_wide_null("Nach Updates suchen");
        let snooze1_lbl = to_wide_null("Neustart um 1 Std. verschieben");
        let snooze4_lbl = to_wide_null("Neustart um 4 Std. verschieben");
        let reboot_lbl = to_wide_null("Jetzt neu starten");
        let quit_lbl = to_wide_null("Beenden");

        AppendMenuW(hmenu, MF_STRING, IDM_OPEN_CENTER, open_lbl.as_ptr());
        AppendMenuW(hmenu, MF_STRING, IDM_CHECK_UPDATES, updates_lbl.as_ptr());
        AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());

        let rb = st
            .shared_reboot
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();
        let now_unix = chrono::Utc::now().timestamp();
        let snooze_enabled = TrayStateTracker::can_snooze_reboot(&rb, now_unix);
        let snooze_flags = if snooze_enabled {
            MF_STRING
        } else {
            MF_STRING | MF_GRAYED
        };

        AppendMenuW(hmenu, snooze_flags, IDM_SNOOZE_1H, snooze1_lbl.as_ptr());
        AppendMenuW(hmenu, snooze_flags, IDM_SNOOZE_4H, snooze4_lbl.as_ptr());

        if rb.reboot_required {
            AppendMenuW(hmenu, MF_STRING, IDM_REBOOT_NOW, reboot_lbl.as_ptr());
        }

        AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(hmenu, MF_STRING, IDM_QUIT, quit_lbl.as_ptr());

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);

        let selected = TrackPopupMenu(
            hmenu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        ) as usize;

        PostMessageW(hwnd, 0, 0, 0);
        DestroyMenu(hmenu);

        match selected {
            IDM_OPEN_CENTER => launch_desktop_window(&st.center_url),
            IDM_CHECK_UPDATES => {
                let _ = st.action_tx.send(TrayMenuAction::CheckForUpdates);
            }
            IDM_SNOOZE_1H => {
                let _ = st.action_tx.send(TrayMenuAction::SnoozeRebootMinutes(60));
            }
            IDM_SNOOZE_4H => {
                let _ = st.action_tx.send(TrayMenuAction::SnoozeRebootMinutes(240));
            }
            IDM_REBOOT_NOW => {
                let _ = st.action_tx.send(TrayMenuAction::RebootNow);
            }
            IDM_QUIT => {
                let _ = st.action_tx.send(TrayMenuAction::QuitTray);
                DestroyWindow(hwnd);
            }
            _ => {}
        }
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAYICON => {
                let event = (lparam as u32) & 0xFFFF;
                match event {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                        TRAY_STATE.with(|cell| {
                            if let Some(st) = cell.borrow().as_ref() {
                                launch_desktop_window(&st.center_url);
                            }
                        });
                    }
                    WM_RBUTTONUP | WM_CONTEXTMENU => {
                        TRAY_STATE.with(|cell| {
                            if let Some(st) = cell.borrow().as_ref() {
                                show_context_menu(hwnd, st);
                            }
                        });
                    }
                    _ => {}
                }
                0
            }
            WM_TRAY_BALLOON => {
                TRAY_STATE.with(|cell| {
                    if let Some(st) = cell.borrow_mut().as_mut() {
                        if let Ok(mut q) = st.balloon_queue.lock() {
                            while let Some(note) = q.pop_front() {
                                let mut nid = st.nid;
                                nid.uFlags = NIF_INFO;
                                nid.dwInfoFlags = if note.urgent {
                                    NIIF_WARNING
                                } else {
                                    NIIF_INFO
                                };
                                copy_wide_fixed(&mut nid.szInfoTitle, &note.title);
                                copy_wide_fixed(&mut nid.szInfo, &note.body);
                                Shell_NotifyIconW(NIM_MODIFY, &nid);
                            }
                        }
                    }
                });
                0
            }
            WM_TRAY_SHUTDOWN => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                TRAY_STATE.with(|cell| {
                    if let Some(mut st) = cell.borrow_mut().take() {
                        if st.icon_added {
                            Shell_NotifyIconW(NIM_DELETE, &st.nid);
                            st.icon_added = false;
                        }
                        if st.hicon != 0 {
                            DestroyIcon(st.hicon);
                        }
                    }
                });
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    pub struct NativeTrayHandle {
        hwnd: isize,
        balloon_queue: Arc<Mutex<VecDeque<DesktopNotification>>>,
    }

    impl NativeTrayHandle {
        pub fn spawn(
            center_url: String,
            shared_reboot: Arc<RwLock<RebootState>>,
            action_tx: UnboundedSender<TrayMenuAction>,
        ) -> Option<Self> {
            let balloon_queue = Arc::new(Mutex::new(VecDeque::new()));
            let queue_for_thread = Arc::clone(&balloon_queue);
            let (hwnd_tx, hwnd_rx) = std::sync::mpsc::sync_channel::<isize>(1);

            let spawn_res = std::thread::Builder::new()
                .name("swm-win32-tray".to_string())
                .spawn(move || unsafe {
                    let hinstance = GetModuleHandleW(std::ptr::null());
                    let class_name = to_wide_null("NS8SwmCenterTrayWindowClass");
                    let window_title = to_wide_null("NS8 Software Center Tray");

                    let wc = WNDCLASSW {
                        style: 0,
                        lpfnWndProc: Some(tray_wnd_proc),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hInstance: hinstance,
                        hIcon: 0,
                        hCursor: 0,
                        hbrBackground: 0,
                        lpszMenuName: std::ptr::null(),
                        lpszClassName: class_name.as_ptr(),
                    };
                    let _ = RegisterClassW(&wc);

                    let hwnd = CreateWindowExW(
                        0,
                        class_name.as_ptr(),
                        window_title.as_ptr(),
                        WS_OVERLAPPEDWINDOW,
                        CW_USEDEFAULT,
                        CW_USEDEFAULT,
                        CW_USEDEFAULT,
                        CW_USEDEFAULT,
                        0,
                        0,
                        hinstance,
                        std::ptr::null(),
                    );
                    if hwnd == 0 {
                        let _ = hwnd_tx.send(0);
                        return;
                    }

                    let hicon = create_shield_cube_hicon();
                    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                    nid.hWnd = hwnd;
                    nid.uID = 1;
                    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
                    nid.uCallbackMessage = WM_TRAYICON;
                    nid.hIcon = hicon;
                    copy_wide_fixed(&mut nid.szTip, "NS8 Software Center");

                    let icon_added = Shell_NotifyIconW(NIM_ADD, &nid) != 0;

                    TRAY_STATE.with(|cell| {
                        *cell.borrow_mut() = Some(Win32TrayThreadState {
                            nid,
                            hicon,
                            center_url,
                            shared_reboot,
                            action_tx,
                            balloon_queue: queue_for_thread,
                            icon_added,
                        });
                    });

                    let _ = hwnd_tx.send(hwnd as isize);

                    let mut msg: MSG = std::mem::zeroed();
                    while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                });

            if spawn_res.is_err() {
                return None;
            }

            match hwnd_rx.recv_timeout(std::time::Duration::from_secs(3)) {
                Ok(hwnd) if hwnd != 0 => Some(Self {
                    hwnd,
                    balloon_queue,
                }),
                _ => None,
            }
        }

        pub fn show_balloon(&self, note: DesktopNotification) {
            if !notifications_enabled() {
                return;
            }
            if let Ok(mut q) = self.balloon_queue.lock() {
                q.push_back(note);
            }
            unsafe {
                let _ = PostMessageW(self.hwnd as HWND, WM_TRAY_BALLOON, 0, 0);
            }
        }

        pub fn shutdown(&self) {
            unsafe {
                let _ = PostMessageW(self.hwnd as HWND, WM_TRAY_SHUTDOWN, 0, 0);
            }
        }
    }
}

#[cfg(windows)]
pub use win32_tray::NativeTrayHandle;

#[cfg(not(windows))]
pub struct NativeTrayHandle;

#[cfg(not(windows))]
impl NativeTrayHandle {
    pub fn spawn(
        _center_url: String,
        _shared_reboot: Arc<RwLock<RebootState>>,
        _action_tx: UnboundedSender<TrayMenuAction>,
    ) -> Option<Self> {
        None
    }

    pub fn show_balloon(&self, _note: DesktopNotification) {}

    pub fn shutdown(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use swm_common::RebootState;

    #[test]
    fn test_tray_minimize_and_reboot_countdown_notifications() {
        let mut tracker = TrayStateTracker::new(false);
        assert!(tracker.on_window_close_requested());
        assert!(tracker.minimized_to_tray);

        let snap = CenterStatusSnapshot {
            device_id: "dev-1".to_string(),
            server_url: "https://swm.local".to_string(),
            agent_version: "1.0.0".to_string(),
            connected: true,
            device_status: "approved".to_string(),
            pending_os_updates: vec![],
            active_jobs: vec![],
            recent_history: vec![],
            reboot_state: RebootState {
                reboot_required: true,
                policy: RebootPolicy::GracePeriod,
                grace_period_minutes: 240,
                detected_at_unix: Some(1000),
                deadline_unix: Some(15400),
                snoozed_until_unix: None,
                triggered_by_job_id: Some("job-1".to_string()),
            },
        };

        // Initial reboot notification fires once
        let n1 = tracker.evaluate_notifications(&snap, 2000);
        assert_eq!(n1.len(), 1);
        assert!(!n1[0].urgent);
        assert!(tracker.evaluate_notifications(&snap, 2005).is_empty());

        // Final warning fires when <= 15 minutes remain before deadline
        let n_urgent = tracker.evaluate_notifications(&snap, 14_800);
        assert_eq!(n_urgent.len(), 1);
        assert!(n_urgent[0].urgent);

        // Menu hides Snooze options once deadline has passed
        let menu_before = TrayStateTracker::available_menu_items(&snap.reboot_state, 5000);
        assert!(menu_before.contains(&TrayMenuAction::SnoozeRebootMinutes(60)));
        let menu_after = TrayStateTracker::available_menu_items(&snap.reboot_state, 15_400);
        assert!(!menu_after.contains(&TrayMenuAction::SnoozeRebootMinutes(60)));
        assert!(menu_after.contains(&TrayMenuAction::RebootNow));
    }

    #[test]
    fn test_procedural_shield_cube_icon_bgra32_geometry() {
        let pixels = render_shield_cube_bgra32();
        assert_eq!(pixels.len(), 32 * 32 * 4);

        // Top-left corner (0, 0) must be fully transparent
        assert_eq!(pixels[3], 0);
        // Center pixel (16, 15) must be opaque inside the shield-cube
        let center_alpha = pixels[(15 * 32 + 16) * 4 + 3];
        assert_eq!(center_alpha, 255);
    }

    #[test]
    fn test_winrt_toast_xml_escaping_and_powershell_builder() {
        let note = DesktopNotification {
            title: "Update <KB5044> & 'Restart'".to_string(),
            body: "Bitte \"Arbeit\" speichern > Neustart in 5m".to_string(),
            urgent: true,
        };
        let xml = build_winrt_toast_xml(&note);
        assert!(xml.contains("template=\"ToastGeneric\""));
        assert!(xml.contains("duration=\"long\""));
        assert!(xml.contains("Update &lt;KB5044&gt; &amp; &apos;Restart&apos;"));
        assert!(xml.contains("Bitte &quot;Arbeit&quot; speichern &gt; Neustart in 5m"));

        let ps = build_powershell_toast_script(&note);
        assert!(ps.contains("ToastNotificationManager"));
        assert!(ps.contains("Windows.Data.Xml.Dom.XmlDocument"));
        assert!(ps.contains(POWERSHELL_TOAST_APP_ID));
        assert!(ps.contains("CreateToastNotifier"));
    }

    #[test]
    fn test_headless_tray_gate_skips_when_no_open_is_true() {
        assert!(!should_spawn_native_tray(true));
    }
}
