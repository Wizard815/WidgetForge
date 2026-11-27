//! Screen time: which app is in front, sampled every few seconds, kept on
//! this computer only (one file, the last 15 days). Sampling only runs while
//! a screen-time widget has asked in the last 15 minutes, and skips time when
//! nobody has touched the mouse or keyboard for 3 minutes.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SAMPLE: Duration = Duration::from_secs(5);
const IDLE_LIMIT: Duration = Duration::from_secs(180);
const STOP_AFTER: Duration = Duration::from_secs(15 * 60);
const KEEP_HOURS: u64 = 15 * 24;

/// hour (Unix seconds / 3600) → app → seconds
type Hours = BTreeMap<u64, BTreeMap<String, u64>>;

struct Tracker {
    file: PathBuf,
    hours: Hours,
    last_asked: Instant,
    running: bool,
    reason: Option<String>,
}

static TRACKER: Mutex<Option<Tracker>> = Mutex::new(None);

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The data for the widget; starts the sampler if it is not running.
pub fn report(file: PathBuf) -> Value {
    let mut guard = TRACKER.lock().unwrap();
    let t = guard.get_or_insert_with(|| {
        let hours: Hours = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Tracker { file, hours, last_asked: Instant::now(), running: false, reason: platform::unsupported() }
    });
    t.last_asked = Instant::now();
    if let Some(reason) = &t.reason {
        return json!({ "available": false, "reason": reason });
    }
    if !t.running {
        t.running = true;
        std::thread::spawn(sampler);
    }
    let mut hours = Map::new();
    for (h, apps) in &t.hours {
        hours.insert(h.to_string(), json!(apps));
    }
    json!({ "available": true, "hours": hours, "sample_seconds": SAMPLE.as_secs() })
}

fn sampler() {
    let mut probe = platform::Probe::new();
    let mut since_save = 0u32;
    loop {
        std::thread::sleep(SAMPLE);
        let app = if probe.idle().is_some_and(|i| i >= IDLE_LIMIT) { None } else { probe.active_app() };
        let mut guard = TRACKER.lock().unwrap();
        let Some(t) = guard.as_mut() else { return };
        if t.last_asked.elapsed() > STOP_AFTER {
            save(t);
            t.running = false;
            return;
        }
        if let Some(app) = app {
            let hour = now_secs() / 3600;
            *t.hours.entry(hour).or_default().entry(app).or_default() += SAMPLE.as_secs();
            let oldest = hour.saturating_sub(KEEP_HOURS);
            t.hours.retain(|h, _| *h >= oldest);
        }
        since_save += 1;
        if since_save >= 12 {
            since_save = 0;
            save(t);
        }
    }
}

fn save(t: &Tracker) {
    if let Some(dir) = t.file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = crate::model::write_json(&t.file, &t.hours);
}

/// Forgets everything recorded so far.
pub fn clear() {
    if let Some(t) = TRACKER.lock().unwrap().as_mut() {
        t.hours.clear();
        save(t);
    }
}

/// "firefox" from "/usr/lib/firefox/firefox" or "Firefox.exe".
pub fn tidy_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let base = base.strip_suffix(".exe").or(base.strip_suffix(".EXE")).unwrap_or(base);
    base.trim().to_string()
}

#[cfg(target_os = "linux")]
mod platform {
    use std::time::Duration;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
    use x11rb::rust_connection::RustConnection;

    pub fn unsupported() -> Option<String> {
        if std::env::var_os("DISPLAY").is_none() {
            return Some("Screen time needs an X11 desktop for now; Wayland desktops do not let apps see which window is in front.".into());
        }
        None
    }

    pub struct Probe {
        conn: Option<(RustConnection, usize)>,
    }

    impl Probe {
        pub fn new() -> Self {
            Self { conn: x11rb::connect(None).ok() }
        }

        pub fn idle(&mut self) -> Option<Duration> {
            use x11rb::protocol::screensaver::ConnectionExt as _;
            let (c, screen) = self.conn.as_ref()?;
            let root = c.setup().roots.get(*screen)?.root;
            let info = c.screensaver_query_info(root).ok()?.reply().ok()?;
            Some(Duration::from_millis(info.ms_since_user_input as u64))
        }

        pub fn active_app(&mut self) -> Option<String> {
            let (c, screen) = self.conn.as_ref()?;
            let root = c.setup().roots.get(*screen)?.root;
            let atom = |name: &[u8]| c.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom);
            let active = atom(b"_NET_ACTIVE_WINDOW")?;
            let reply = c.get_property(false, root, active, AtomEnum::WINDOW, 0, 1).ok()?.reply().ok()?;
            let win = reply.value32()?.next()?;
            if win == 0 {
                return None;
            }
            // WM_CLASS is "instance\0Class\0"; the class reads best ("Firefox").
            let class = c.get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256).ok()?.reply().ok()?;
            let parts: Vec<String> = class.value.split(|b| *b == 0).filter(|p| !p.is_empty()).map(|p| String::from_utf8_lossy(p).into_owned()).collect();
            parts.get(1).or(parts.first()).map(|s| super::tidy_name(s))
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::time::Duration;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    pub fn unsupported() -> Option<String> {
        None
    }

    pub struct Probe;

    impl Probe {
        pub fn new() -> Self {
            Probe
        }

        pub fn idle(&mut self) -> Option<Duration> {
            let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
            // SAFETY: plain Win32 call with a correctly sized struct.
            if unsafe { GetLastInputInfo(&mut info) } == 0 {
                return None;
            }
            let now = unsafe { GetTickCount() };
            Some(Duration::from_millis(now.wrapping_sub(info.dwTime) as u64))
        }

        pub fn active_app(&mut self) -> Option<String> {
            // SAFETY: Win32 calls; the handle is closed before returning.
            unsafe {
                let hwnd = GetForegroundWindow();
                if hwnd.is_null() {
                    return None;
                }
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, &mut pid);
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if h.is_null() {
                    return None;
                }
                let mut buf = [0u16; 1024];
                let mut len = buf.len() as u32;
                let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
                CloseHandle(h);
                if ok == 0 {
                    return None;
                }
                Some(super::tidy_name(&String::from_utf16_lossy(&buf[..len as usize])))
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    use std::time::Duration;
    pub fn unsupported() -> Option<String> {
        Some("Screen time works on Linux (X11) and Windows so far.".into())
    }
    pub struct Probe;
    impl Probe {
        pub fn new() -> Self {
            Probe
        }
        pub fn idle(&mut self) -> Option<Duration> {
            None
        }
        pub fn active_app(&mut self) -> Option<String> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_names() {
        assert_eq!(super::tidy_name("/usr/lib/firefox/firefox"), "firefox");
        assert_eq!(super::tidy_name("C:\\Program Files\\Steam\\steam.exe"), "steam");
        assert_eq!(super::tidy_name("Code"), "Code");
    }
}
