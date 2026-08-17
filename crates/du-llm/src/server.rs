//! Сайдкар llama-server (llama.cpp). Поднимаем один процесс на свободном порту 127.0.0.1,
//! ждём готовности по `/health`, глушим при `Drop`.
//!
//! Резидентная модель держит VRAM, поэтому убийство процесса — это и есть освобождение
//! карты под генерацию картинки. На одной 4090 текст и картинка не живут одновременно.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::LlmError;

/// Хвост последних строк вывода — чтобы в ошибке было видно, почему сервер не поднялся.
type LogTail = Arc<Mutex<VecDeque<String>>>;
const TAIL_LINES: usize = 40;

fn drain_to_tail<R: std::io::Read + Send + 'static>(reader: R, tail: LogTail) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if let Ok(mut tail) = tail.lock() {
                if tail.len() >= TAIL_LINES {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        }
    });
}

fn tail_text(tail: &LogTail) -> String {
    tail.lock()
        .map(|tail| tail.iter().cloned().collect::<Vec<_>>().join(" | "))
        .unwrap_or_default()
}

pub struct ServerOpts {
    pub bin: PathBuf,
    /// GGUF основной модели (нарратор).
    pub model: PathBuf,
    /// Проектор для мультимодальности: без него портреты персонажей модель не увидит.
    pub mmproj: Option<PathBuf>,
    /// Слои на GPU; −1 = все.
    pub n_gpu_layers: i32,
    pub ctx_size: u32,
    pub ready_timeout_secs: u64,
}

impl ServerOpts {
    pub fn new(bin: impl Into<PathBuf>, model: impl Into<PathBuf>) -> Self {
        Self {
            bin: bin.into(),
            model: model.into(),
            mmproj: None,
            n_gpu_layers: -1,
            // 16k хватает: в промпт уезжает около 48 тысяч символов истории, это примерно
            // 13 тысяч токенов. Больший контекст даром съедает VRAM под KV-кэш, а карту мы
            // делим с картинками и озвучкой.
            ctx_size: 16_384,
            ready_timeout_secs: 300,
        }
    }

    pub fn with_mmproj(mut self, mmproj: Option<PathBuf>) -> Self {
        self.mmproj = mmproj;
        self
    }

    pub fn with_ctx(mut self, ctx_size: u32) -> Self {
        if ctx_size > 0 {
            self.ctx_size = ctx_size;
        }
        self
    }
}

pub struct LlamaServer {
    child: Child,
    port: u16,
    base_url: String,
    log_tail: LogTail,
}

/// Свободный TCP-порт на 127.0.0.1. Между освобождением и стартом сервера остаётся тонкая
/// гонка, но для локального сайдкара это приемлемо.
fn free_port() -> Result<u16, LlmError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| LlmError::Spawn(format!("не занять порт: {e}")))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| LlmError::Spawn(format!("не прочитать порт: {e}")))
}

impl LlamaServer {
    pub fn start(opts: &ServerOpts) -> Result<Self, LlmError> {
        if !opts.bin.is_file() {
            return Err(LlmError::Spawn(format!("llama-server не найден: {}", opts.bin.display())));
        }
        if !opts.model.is_file() {
            return Err(LlmError::Spawn(format!("модель не найдена: {}", opts.model.display())));
        }
        let port = free_port()?;
        let mut cmd = Command::new(&opts.bin);
        cmd.arg("-m")
            .arg(&opts.model)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-ngl")
            .arg(opts.n_gpu_layers.to_string())
            .arg("-c")
            .arg(opts.ctx_size.to_string())
            .arg("--flash-attn")
            .arg("on")
            // Один запрос за раз: пайплайн последовательный, а параллельные слоты только
            // раздували бы KV-кэш на карте, которую мы делим с картинками.
            .arg("--parallel")
            .arg("1")
            // Без Jinja сервер игнорирует chat_template_kwargs, модель уходит «думать»,
            // сжигает лимит токенов на рассуждения и возвращает ПУСТОЙ content.
            .arg("--jinja");
        if let Some(mmproj) = opts.mmproj.as_ref().filter(|path| path.is_file()) {
            cmd.arg("--mmproj").arg(mmproj);
        }
        // Оба конвейера обязательно вычитываем: непрочитанный буфер переполнится, и сервер
        // заблокируется на записи, так и не дойдя до готовности.
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| LlmError::Spawn(format!("не запустить llama-server: {e}")))?;
        // Привязываем модель к нашему заданию. Уборка сирот при старте — лечение по факту:
        // между падением и следующим запуском процесс живёт и держит видеопамять, а если
        // запусков было несколько, их набирается четыре штуки на двадцать гигабайт. С
        // заданием такого просто не может случиться: система убивает детей вместе с нами,
        // даже когда нас сняли силой и до штатного закрытия дело не дошло.
        adopt_into_job(&child);
        let log_tail: LogTail = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(stdout) = child.stdout.take() {
            drain_to_tail(stdout, log_tail.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            drain_to_tail(stderr, log_tail.clone());
        }

        let mut server = LlamaServer {
            child,
            port,
            base_url: format!("http://127.0.0.1:{port}"),
            log_tail,
        };
        server.wait_ready(opts.ready_timeout_secs)?;
        Ok(server)
    }

    /// Опрос `/health` до готовности. Если процесс уже умер (нет VRAM, битый GGUF, занятый
    /// порт), выходим сразу с хвостом лога, а не ждём весь таймаут.
    fn wait_ready(&mut self, timeout_secs: u64) -> Result<(), LlmError> {
        let health = format!("{}/health", self.base_url);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| LlmError::Http(e.to_string()))?;
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(LlmError::Spawn(format!(
                    "llama-server завершился до готовности ({status}); вывод: {}",
                    tail_text(&self.log_tail)
                )));
            }
            if let Ok(response) = client.get(&health).send() {
                if response.status().is_success() {
                    let ready = response
                        .json::<serde_json::Value>()
                        .map(|body| body.get("status").and_then(|s| s.as_str()) == Some("ok"))
                        // Двухсотка без тела тоже считается готовностью.
                        .unwrap_or(true);
                    if ready {
                        return Ok(());
                    }
                }
            }
            if Instant::now() >= deadline {
                return Err(LlmError::Spawn(format!(
                    "llama-server не поднялся за {timeout_secs}с на порту {}; вывод: {}",
                    self.port,
                    tail_text(&self.log_tail)
                )));
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Остановить сервер и вернуть VRAM. Идемпотентно.
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Убить процессы модели, оставшиеся от прошлого запуска.
///
/// `Drop` закрывает свой процесс, но только при штатном выходе. Если игру сняли силой или
/// она упала, дочерний `llama-server` остаётся жить и держит гигабайты видеопамяти: у меня
/// на карте набралось три таких, 23.7 ГБ из 24.5, и всё после этого ползло. Поэтому при
/// старте подчищаем чужих сирот — тех, чей путь ведёт в наш каталог инструментов.
pub fn kill_orphans(tools_dir: &Path) {
    let ours = tools_dir.canonicalize().unwrap_or_else(|_| tools_dir.to_path_buf());
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mine = std::process::id();
    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if !name.starts_with("llama-server") {
            continue;
        }
        // Свой собственный процесс и чужие сборки не трогаем: убиваем только то, что
        // запускали мы, из нашего каталога.
        let from_us = process
            .exe()
            .and_then(|path| path.parent().map(|dir| dir.starts_with(&ours)))
            .unwrap_or(false);
        if from_us && pid.as_u32() != mine {
            tracing::info!("убираю оставшийся llama-server {pid}");
            process.kill();
        }
    }
}

/// Задание, в котором живут все запущенные нами модели.
///
/// Одно на процесс: система закрывает его дескриптор при нашей смерти — и убивает всё, что
/// внутри. Создаётся лениво, потому что до первой модели оно не нужно.
#[cfg(windows)]
fn job_handle() -> isize {
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, SetInformationJobObject, JobObjectExtendedLimitInformation,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    static JOB: OnceLock<isize> = OnceLock::new();
    *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job == 0 {
            tracing::warn!("не удалось создать задание для процессов модели");
            return 0;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 {
            tracing::warn!("задание создано, но правило «умереть вместе с нами» не поставилось");
        }
        job
    })
}

/// Взять запущенный процесс в наше задание.
#[cfg(windows)]
fn adopt_into_job(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;

    let job = job_handle();
    if job == 0 {
        return;
    }
    // Дескриптор процесса у Command уже есть — второй раз открывать его не нужно.
    let handle = child.as_raw_handle() as isize;
    let ok = unsafe { AssignProcessToJobObject(job, handle) };
    if ok == 0 {
        tracing::warn!("модель не удалось привязать к заданию: сирота переживёт нас");
    }
}

/// На прочих системах ничего не делаем: там процессы и так не остаются.
#[cfg(not(windows))]
fn adopt_into_job(_child: &Child) {}

/// Найти `llama-server` в каталоге инструментов.
pub fn resolve_llama_bin(tools_dir: &Path) -> PathBuf {
    if let Ok(path) = std::env::var("DU_LLAMA_BIN") {
        return PathBuf::from(path);
    }
    tools_dir.join(if cfg!(windows) { "llama-server.exe" } else { "llama-server" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_binary_is_reported_before_anything_is_spawned() {
        let opts = ServerOpts::new("нет-такого-бинаря", "нет-такой-модели");
        let error = LlamaServer::start(&opts).map(|_| ()).unwrap_err();
        assert!(matches!(error, LlmError::Spawn(ref text) if text.contains("llama-server не найден")));
    }

    #[test]
    fn free_ports_are_actually_free() {
        let port = free_port().unwrap();
        assert!(port > 0);
        // Порт отпущен, значит его можно занять снова.
        assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    }

    #[test]
    fn the_context_size_ignores_a_zero_override() {
        let opts = ServerOpts::new("bin", "model").with_ctx(0);
        assert_eq!(opts.ctx_size, 16_384);
        assert_eq!(ServerOpts::new("bin", "model").with_ctx(8192).ctx_size, 8192);
    }
}
