//! The game's log (Speech Studio `app/diagnostics.rs`): the console, the latest lines kept for the report, and the log
//! on disk, `<root>/logs/dungeon-ultimate.log`, with panics and, on Windows, exceptions nobody handles.
//! Keys never get in: anything in the log that looks like one is masked.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

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
}
