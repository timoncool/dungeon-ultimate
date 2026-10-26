//! The game's log and diagnostics (Speech Studio `app/diagnostics.rs`): the console, the latest lines kept for the
//! report, the log on disk, `<root>/logs/dungeon-ultimate.log`, with panics and, on Windows, exceptions nobody handles;
//! the version, the system, the cards, the components and the voice server's missing libraries as text to copy, and
//! as a bug-report pack (`<root>/reports/*.zip`: the diagnostics, the activity journal and the log).
//! Keys never get in: they are not in the report, and anything in the log that looks like one is masked.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;

use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// How many of the server's latest log lines are kept for the report.
const LOG_LINES: usize = 400;

static LOG: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// A tracing layer keeping the latest log lines (info and above) for the report and writing them to the log file.
pub struct RecentLog;

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}{}", self.0);
        } else {
            self.0.push_str(&format!(" {}={value:?}", field.name()));
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.0 = format!("{value}{}", self.0);
        } else {
            self.0.push_str(&format!(" {}={value}", field.name()));
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for RecentLog {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let meta = event.metadata();
        if *meta.level() > tracing::Level::INFO {
            return;
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        let line = masked(&format!("{} {:>5} {}: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), meta.level(), meta.target(), message.0));
        to_file(&line);
        let mut lines = LOG.lock().unwrap_or_else(|p| p.into_inner());
        lines.push_back(line);
        while lines.len() > LOG_LINES {
            lines.pop_front();
        }
    }
}

/// The process's logging: the console (info and above, RUST_LOG overrides) and the latest lines kept for the report.
/// The server binary and the desktop app call it once at start.
pub fn init_logging() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let started = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .with(RecentLog)
        .try_init();
    if let Err(e) = started {
        eprintln!("[ERROR] logging was set up before: {e}");
    }
}

/// The log on disk: a crash leaves no other trace, so every line goes straight to the file (no buffer of ours to
/// lose). Past `LOG_FILE_MAX` the file becomes `dungeon-ultimate.old.log`, replacing the older.
pub const LOG_FILE: &str = "dungeon-ultimate.log";
pub const LOG_FILE_OLD: &str = "dungeon-ultimate.old.log";
const LOG_FILE_MAX: u64 = 8 * 1024 * 1024;

struct LogSink {
    file: std::fs::File,
    written: u64,
}

static SINK: Mutex<Option<LogSink>> = Mutex::new(None);
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Starts writing the log to `<root>/logs` and records crashes there: a panic of any thread and, on Windows, an
/// exception nobody handles (an engine DLL running an instruction this processor lacks, a bad access) with its code
/// and the module it came from. The server binary and the desktop app call it once the data folder is known.
pub fn log_to_file(root: &Path) {
    let dir = root.join("logs");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::error!("[ERROR] {}: the log folder was not created, the log stays in memory only: {e}", dir.display());
        return;
    }
    if LOG_DIR.set(dir.clone()).is_err() {
        return;
    }
    match open_sink(&dir, true) {
        Ok(sink) => *SINK.lock().unwrap_or_else(|p| p.into_inner()) = Some(sink),
        Err(e) => {
            tracing::error!("[ERROR] {}: the log file was not opened, the log stays in memory only: {e}", dir.join(LOG_FILE).display());
            return;
        }
    }
    to_file(&format!("==== Dungeon Ultimate {} started {} (pid {})", env!("CARGO_PKG_VERSION"), chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), std::process::id()));
    let earlier = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        append_apart(&format!("{} [PANIC] thread {}: {info}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), thread.name().unwrap_or("?")));
        earlier(info);
    }));
    #[cfg(windows)]
    crash::watch();
}

/// The log file, the one of the last start moved aside when it has grown past `LOG_FILE_MAX` (`rotate_big`).
fn open_sink(dir: &Path, rotate_big: bool) -> std::io::Result<LogSink> {
    let path = dir.join(LOG_FILE);
    if rotate_big && std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_FILE_MAX) {
        std::fs::rename(&path, dir.join(LOG_FILE_OLD))?;
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    let written = file.metadata().map(|m| m.len()).unwrap_or(0);
    Ok(LogSink { file, written })
}

/// One line to the log file through a handle of its own: a panicking or crashing thread may hold the log's lock.
fn append_apart(line: &str) {
    let Some(dir) = LOG_DIR.get() else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(LOG_FILE)) {
        let _ = writeln!(f, "{line}");
    }
}

/// One line to the log file, when there is one; past `LOG_FILE_MAX` the file is moved aside and a new one begins.
fn to_file(line: &str) {
    let Some(dir) = LOG_DIR.get() else { return };
    let mut sink = SINK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(s) = sink.as_mut() else { return };
    if s.written > LOG_FILE_MAX {
        *sink = None;
        match open_sink(dir, true) {
            Ok(fresh) => *sink = Some(fresh),
            Err(e) => {
                eprintln!("[ERROR] {}: the log file was not rotated: {e}", dir.join(LOG_FILE).display());
                return;
            }
        }
    }
    let Some(s) = sink.as_mut() else { return };
    let bytes = format!("{line}\n");
    match s.file.write_all(bytes.as_bytes()) {
        Ok(()) => s.written += bytes.len() as u64,
        Err(e) => eprintln!("[ERROR] the log file did not take a line: {e}"),
    }
}

#[cfg(windows)]
mod crash {
    use windows_sys::Win32::Foundation::{EXCEPTION_ACCESS_VIOLATION, EXCEPTION_ILLEGAL_INSTRUCTION, EXCEPTION_STACK_OVERFLOW, HMODULE};
    use windows_sys::Win32::System::Diagnostics::Debug::{SetUnhandledExceptionFilter, EXCEPTION_POINTERS};
    use windows_sys::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT};

    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

    pub fn watch() {
        // SAFETY: the filter is a plain function that lives as long as the process.
        unsafe { SetUnhandledExceptionFilter(Some(on_crash)) };
    }

    /// The module holding `addr` and the offset in it, as the Windows event log names a crash.
    fn module_of(addr: usize) -> (String, usize) {
        let mut module: HMODULE = std::ptr::null_mut();
        // SAFETY: the address is only looked up; the module's refcount is left as it is.
        let found = unsafe { GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, addr as *const u16, &mut module) } != 0;
        if !found {
            return ("?".to_string(), addr);
        }
        let mut name = [0u16; 520];
        // SAFETY: the buffer's length is passed with it.
        let n = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) } as usize;
        (String::from_utf16_lossy(&name[..n.min(name.len())]), addr - module as usize)
    }

    unsafe extern "system" fn on_crash(info: *const EXCEPTION_POINTERS) -> i32 {
        // SAFETY: Windows passes the record of the exception being raised.
        let record = unsafe { info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) };
        let Some(record) = record else { return EXCEPTION_CONTINUE_SEARCH };
        let code = record.ExceptionCode;
        let what = match code {
            EXCEPTION_ILLEGAL_INSTRUCTION => " (an instruction this processor does not have)",
            EXCEPTION_ACCESS_VIOLATION => " (access violation)",
            EXCEPTION_STACK_OVERFLOW => " (stack overflow)",
            _ => "",
        };
        let (module, offset) = module_of(record.ExceptionAddress as usize);
        let line = format!(
            "{} [CRASH] exception 0x{:08x}{what} in {module} at offset 0x{offset:x}, thread {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            code as u32,
            std::thread::current().name().unwrap_or("?"),
        );
        super::append_apart(&line);
        EXCEPTION_CONTINUE_SEARCH
    }
}

/// Anything that looks like a key masked: OpenRouter, Google, fal, Hugging Face, bearer headers.
fn masked(text: &str) -> String {
    static KEYS: OnceLock<regex::Regex> = OnceLock::new();
    let keys = KEYS.get_or_init(|| regex::Regex::new(r"(sk-[A-Za-z0-9_-]{8,}|AIza[0-9A-Za-z_-]{20,}|hf_[A-Za-z0-9]{20,}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:[0-9a-f]{20,}|(?i:bearer)\s+[A-Za-z0-9._-]{12,})").expect("key pattern"));
    keys.replace_all(text, "[key]").into_owned()
}

/// The latest log lines, oldest first.
pub fn recent_log() -> Vec<String> {
    LOG.lock().unwrap_or_else(|p| p.into_inner()).iter().cloned().collect()
}

/// The diagnostics as data (Speech Studio `app/diagnostics.rs`): the version, the system, the cards and the build the
/// game runs on, memory, every component and whether it is on disk, what the voice server's libraries miss.
fn report(root: &Path) -> Value {
    let machine = crate::accel::machine();
    let runtime = crate::runtime::load(root);
    let accel = crate::accel::accel(&runtime);
    let hw = crate::hw::snapshot();
    let components: Vec<Value> = crate::setup::status(root)
        .iter()
        .map(|c| json!({ "id": c.component.id, "installed": c.present, "size": c.component.bytes(), "on_disk": c.have_bytes }))
        .collect();
    let build = crate::acpp::build(accel);
    let exe = crate::acpp::exe(root, build);
    let dirs: Vec<PathBuf> = exe.parent().map(Path::to_path_buf).into_iter().chain(crate::acpp::library_dir(root, build)).collect();
    let dir_refs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    let voice_missing = if exe.is_file() { acpp_server::asset::missing_libraries(build, &dir_refs) } else { Vec::new() };
    let cards: Vec<Value> = machine
        .nvidia
        .iter()
        .map(|card| json!({ "index": card.index, "name": card.name, "vram_gb": card.vram_gb, "compute": format!("{}.{}", card.compute.0, card.compute.1) }))
        .collect();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "os": format!("{} ({})", sysinfo::System::long_os_version().or_else(sysinfo::System::name).unwrap_or_else(|| std::env::consts::OS.to_string()), std::env::consts::ARCH),
        "cards": cards,
        "driver_major": machine.driver_major,
        "other_adapter": machine.other_adapter,
        "accel": accel.key(),
        "narrator_accel": crate::accel::narrator_accel(accel).key(),
        "memory": { "vram_total": hw.total_vram, "vram_free": hw.free_vram, "ram_total": hw.total_ram, "ram_used": hw.used_ram },
        "free_bytes": crate::setup::free_bytes(&root.join("models")),
        "components": components,
        "voice_libraries_missing": voice_missing,
        "data_dir": root.to_string_lossy(),
    })
}

/// The diagnostics as text to paste into an issue.
fn text(report: &Value, log: &[String]) -> String {
    let gb = |b: &Value| b.as_u64().map(|b| format!("{:.1} GB", b as f64 / 1e9)).unwrap_or_else(|| "?".into());
    let mut out = String::new();
    out.push_str(&format!("Dungeon Ultimate {}\n", report["version"].as_str().unwrap_or_default()));
    out.push_str(&format!("OS: {}\n", report["os"].as_str().unwrap_or_default()));
    let cards = report["cards"].as_array().cloned().unwrap_or_default();
    if cards.is_empty() {
        out.push_str("NVIDIA: none\n");
    }
    for card in &cards {
        out.push_str(&format!(
            "GPU {}: {} · {} GB · CC {} · driver {}\n",
            card["index"],
            card["name"].as_str().unwrap_or("?"),
            card["vram_gb"],
            card["compute"].as_str().unwrap_or("?"),
            report["driver_major"].as_u64().map(|d| d.to_string()).unwrap_or_else(|| "?".into()),
        ));
    }
    if let Some(other) = report["other_adapter"].as_str() {
        out.push_str(&format!("Other adapter: {other}\n"));
    }
    out.push_str(&format!("Build: {} · narrator: {}\n", report["accel"].as_str().unwrap_or("?"), report["narrator_accel"].as_str().unwrap_or("?")));
    out.push_str(&format!("VRAM: {} free of {} · RAM: {} used of {}\n", gb(&report["memory"]["vram_free"]), gb(&report["memory"]["vram_total"]), gb(&report["memory"]["ram_used"]), gb(&report["memory"]["ram_total"])));
    out.push_str(&format!("Data: {} · free {}\n", report["data_dir"].as_str().unwrap_or_default(), gb(&report["free_bytes"])));
    out.push_str("Components:\n");
    for c in report["components"].as_array().into_iter().flatten() {
        out.push_str(&format!("  {} {}\n", if c["installed"] == true { "[OK]" } else { "[--]" }, c["id"].as_str().unwrap_or_default()));
    }
    let missing = report["voice_libraries_missing"].as_array().cloned().unwrap_or_default();
    out.push_str(&format!("Voice server libraries: {}\n", if missing.is_empty() { "nothing missing".to_string() } else { missing.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; ") }));
    out.push_str(&format!("\nLatest log ({} lines):\n", log.len()));
    for line in log {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// GET /api/diagnostics: the report and its text.
pub async fn get(State(state): State<AppState>) -> Response {
    let root = state.root.clone();
    match tokio::task::spawn_blocking(move || report(&root)).await {
        Ok(report) => {
            let log = recent_log();
            Json(json!({ "text": text(&report, &log), "report": report })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn pack(root: &Path) -> Result<PathBuf, String> {
    let report = report(root);
    let log = recent_log();
    let dir = root.join("reports");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("dungeon-ultimate-report-{}.zip", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut put = |name: &str, body: &[u8]| -> Result<(), String> {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(body).map_err(|e| e.to_string())
    };
    put("diagnostics.txt", text(&report, &log).as_bytes())?;
    put("diagnostics.json", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?.as_bytes())?;
    if let Ok(journal) = std::fs::read_to_string(root.join("activity.jsonl")) {
        put("activity.jsonl", masked(&journal).as_bytes())?;
    }
    for name in [LOG_FILE_OLD, LOG_FILE] {
        if let Ok(log) = std::fs::read(root.join("logs").join(name)) {
            put(name, masked(&String::from_utf8_lossy(&log)).as_bytes())?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(path)
}

/// POST /api/diagnostics/report: packs the bug report into the data folder and answers its path.
pub async fn make_report(State(state): State<AppState>) -> Response {
    let root = state.root.clone();
    match tokio::task::spawn_blocking(move || pack(&root)).await {
        Ok(Ok(path)) => Json(json!({ "path": path.to_string_lossy() })).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "report_failed", "detail": e }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Shows a path in the computer's file manager (from an AppImage without the image's libraries).
fn reveal(path: &Path) {
    #[cfg(windows)]
    let opened = dub_core::proc::host_command("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let opened = dub_core::proc::host_command("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let opened = dub_core::proc::host_command("xdg-open").arg(path).spawn();
    if let Err(e) = opened {
        tracing::error!("[ERROR] {} was not shown in the file manager: {e}", path.display());
    }
}

/// POST /api/diagnostics/report/reveal {path}: shows a report of the data folder in the file manager.
pub async fn reveal_report(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let dir = state.root.join("reports");
    let asked = PathBuf::from(body.get("path").and_then(Value::as_str).unwrap_or_default());
    let inside = asked.parent().is_some_and(|p| p == dir) && asked.extension().is_some_and(|e| e == "zip") && asked.is_file();
    if !inside {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "no_report", "detail": asked.to_string_lossy() }))).into_response();
    }
    reveal(&dir);
    Json(json!({ "ok": true })).into_response()
}

/// POST /api/diagnostics/logs/reveal: the folder with the game's and its helpers' logs in the file manager.
pub async fn reveal_logs(State(state): State<AppState>) -> Response {
    let dir = state.root.join("logs");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "io", "detail": format!("{}: {e}", dir.display()) }))).into_response();
    }
    reveal(&dir);
    Json(json!({ "ok": true })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_masked_in_the_log() {
        assert_eq!(masked("key sk-or-v1-abcdef1234567890 used"), "key [key] used");
        assert_eq!(masked("Authorization: Bearer abcdefghijklmnop"), "Authorization: [key]");
        assert_eq!(masked("AIzaSyA1234567890abcdefghijk ok"), "[key] ok");
        assert_eq!(masked("ход 12 за 3.2 с"), "ход 12 за 3.2 с");
    }

    #[test]
    fn a_log_grown_past_its_limit_is_moved_aside_at_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOG_FILE), vec![b'x'; LOG_FILE_MAX as usize + 1]).unwrap();
        std::fs::write(dir.path().join(LOG_FILE_OLD), b"older").unwrap();
        let sink = open_sink(dir.path(), true).unwrap();
        assert_eq!(sink.written, 0);
        assert_eq!(std::fs::metadata(dir.path().join(LOG_FILE_OLD)).unwrap().len(), LOG_FILE_MAX + 1, "the older one is replaced");
        drop(sink);
        std::fs::write(dir.path().join(LOG_FILE), b"small").unwrap();
        let sink = open_sink(dir.path(), true).unwrap();
        assert_eq!(sink.written, 5, "a small log is kept and appended to");
    }

    #[test]
    fn the_text_names_the_card_the_builds_and_every_component() {
        let report = json!({ "version": "1.0.0", "os": "Windows 11", "cards": [{ "index": 0, "name": "RTX 4090", "vram_gb": 24.0, "compute": "8.9" }], "driver_major": 617,
            "accel": "cuda13", "narrator_accel": "cuda13",
            "memory": { "vram_total": 24_000_000_000u64, "vram_free": 20_000_000_000u64, "ram_total": 64_000_000_000u64, "ram_used": 10_000_000_000u64 },
            "data_dir": "D:/g", "free_bytes": 100_000_000_000u64,
            "components": [{ "id": "narrator", "installed": true }, { "id": "image-dit", "installed": false }], "voice_libraries_missing": [] });
        let t = text(&report, &["a log line".into()]);
        assert!(t.contains("RTX 4090") && t.contains("driver 617") && t.contains("Build: cuda13"), "{t}");
        assert!(t.contains("[OK] narrator") && t.contains("[--] image-dit") && t.contains("nothing missing") && t.contains("a log line"), "{t}");
    }
}
