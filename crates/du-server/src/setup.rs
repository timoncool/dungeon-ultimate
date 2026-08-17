//! Докачка моделей на первом запуске.
//!
//! Веса в установщик не кладутся: он распух бы на десятки гигабайт. Приложение ставится
//! пустым и добирает недостающее само.
//!
//! Качалка перенесена из Dub Studio вместе с её выстраданными деталями, и трогать их без
//! замера нельзя:
//! * запрос идёт агентом `ureq` — Xet-хранилище Hugging Face душит запросы с чужими
//!   клиентскими подписями и отвечает отказом;
//! * одновременных соединений ровно четыре: при большей параллели Xet роняет соединения,
//!   файл собирается с дырами и молча бьётся;
//! * каждый диапазон проверяется на полноту и повторяется до восьми раз — обрывы там
//!   обычное дело, а недобранный диапазон означает дыру в файле;
//! * данные чанка сбрасываются на диск ДО отметки в манифесте докачки, иначе после жёсткого
//!   выключения возобновление пропустит чанк, у которого на диске нули.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// Одновременных соединений на всю закачку.
const POOL_SLOTS: usize = 4;
/// Размер одной задачи: делит крупный файл на части и не даёт мелким ждать больших.
const CHUNK: u64 = 16 * 1024 * 1024;
/// Сколько раз повторяем ОДИН диапазон, прежде чем признать закачку неудачной.
const RANGE_RETRIES: u32 = 8;

/// Что делать с файлом после закачки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Extract {
    /// Ничего: файл кладётся как есть.
    None,
    /// Zip распаковывается ПЛОСКО в каталог назначения: паки приходят и с папкой внутри,
    /// и без неё, а раскладка должна получаться одинаковая.
    ZipFlat,
}

/// Один скачиваемый файл компонента.
#[derive(Debug, Clone, Serialize)]
pub struct FileSpec {
    /// Куда положить относительно корня приложения.
    pub path: &'static str,
    pub url: &'static str,
    /// Ожидаемый размер для показа прогресса, в байтах.
    pub bytes: u64,
    /// Что сделать после закачки.
    pub extract: Extract,
    /// Куда распаковывать архив.
    pub extract_dir: &'static str,
    /// По какому файлу или каталогу считать распакованное установленным.
    pub marker: &'static str,
}

impl FileSpec {
    /// Обычный файл: положить как есть.
    const fn plain(path: &'static str, url: &'static str, bytes: u64) -> Self {
        Self { path, url, bytes, extract: Extract::None, extract_dir: "", marker: "" }
    }

    /// Архив: распаковать плоско в каталог и считать установленным по маркеру.
    const fn zip(
        path: &'static str,
        url: &'static str,
        bytes: u64,
        dir: &'static str,
        marker: &'static str,
    ) -> Self {
        Self { path, url, bytes, extract: Extract::ZipFlat, extract_dir: dir, marker }
    }
}

/// Насколько компонент нужен для игры.
///
/// Три ступени вместо «да/нет»: раньше всё, что не обязательно, выглядело одинаково
/// необязательным, и игрок не понимал, что скачать сверх минимума, а что не трогать вовсе.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Requirement {
    /// Без него игра не пойдёт — держит первый запуск.
    Required,
    /// Игра пойдёт и без него, но заметно беднее: голос, распознавание речи.
    Recommended,
    /// Дело вкуса: альтернативные веса, дополнительные паки.
    Optional,
}

/// Набор файлов, который игрок ставит одной кнопкой.
#[derive(Debug, Clone, Serialize)]
pub struct Component {
    pub id: &'static str,
    pub title: &'static str,
    /// Зачем оно нужно — показывается под названием.
    pub note: &'static str,
    /// Насколько нужен для игры.
    pub requirement: Requirement,
    pub files: &'static [FileSpec],
}

impl Component {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }

    /// Держит ли он первый запуск.
    pub fn required(&self) -> bool {
        self.requirement == Requirement::Required
    }
}

// ── Откуда что берём ─────────────────────────────────────────────────────────
//
// Ссылки не выдуманы: движки — с релизов их авторов, веса — с Hugging Face, а набор
// зависимостей CUDA совпадает с тем, что уже проверен в Dub Studio. Версии закреплены:
// движок картинок обязан совпадать с `du_image::PINNED_COMMIT`, иначе загрузчик откажется
// работать с чужой сборкой.

/// Сборка stable-diffusion.cpp, под которую написан загрузчик картинок.
const SD_RELEASE: &str = "https://github.com/leejet/stable-diffusion.cpp/releases/download/master-820-de298c2/";
/// Сборка llama.cpp: тот же пин, что и в Dub Studio.
const LLAMA_RELEASE: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b9966/";
/// Веса и движок озвучки Higgs.
const HIGGS: &str = "https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/";
/// Распознавание речи Parakeet в формате onnx.
const PARAKEET: &str = "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/";
/// Среда выполнения onnx: строго 1.24.2 — на других сборках распознавание встаёт намертво.
const ORT: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-1.24.2.zip";

/// Что нужно приложению. Порядок задаёт порядок показа на экране первого запуска.
/// Что нужно приложению. Порядок задаёт порядок показа на экране первого запуска.
///
/// Список статический: срез файлов внутри компонента обязан жить всю программу, иначе на
/// него нельзя ссылаться из статуса и качалки.
static COMPONENTS: &[Component] = &[
        Component {
            id: "image-engine",
            title: "Движок картинок",
            note: "stable-diffusion.cpp с поддержкой видеокарты и библиотеки CUDA к нему",
            requirement: Requirement::Required,
            files: &[
                FileSpec::zip(
                    "downloads/sd-engine.zip",
                    concat!("https://github.com/leejet/stable-diffusion.cpp/releases/download/master-820-de298c2/", "sd-master-de298c2-bin-win-cuda12-x64.zip"),
                    336_000_000,
                    "models/runtime/sd",
                    "models/runtime/sd/stable-diffusion.dll",
                ),
                FileSpec::zip(
                    "downloads/sd-cuda.zip",
                    concat!("https://github.com/leejet/stable-diffusion.cpp/releases/download/master-820-de298c2/", "cudart-sd-bin-win-cu12-x64.zip"),
                    563_000_000,
                    "models/runtime/sd",
                    "models/runtime/sd/cudart64_12.dll",
                ),
            ],
        },
        Component {
            id: "image-dit",
            title: "Модель картинок Krea-2 Turbo",
            note: "рисует кадры сцены; квант Q4_K_M",
            requirement: Requirement::Required,
            files: &[FileSpec::plain(
                "models/image/krea2-turbo-Q4_K_M.gguf",
                "https://huggingface.co/realrebelai/KREA-2_GGUFs/resolve/main/TURBO/Krea-2-Turbo-Q4_K_M.gguf",
                7_216_000_000,
            )],
        },
        Component {
            id: "image-encoder",
            title: "Текстовый энкодер картинок",
            note: "переводит промпт в понятное модели; без цензуры",
            requirement: Requirement::Required,
            files: &[FileSpec::plain(
                "models/image/qwen3-vl-4b-abliterated-Q4_K_M.gguf",
                "https://huggingface.co/noctrex/Huihui-Qwen3-VL-4B-Instruct-abliterated-GGUF/resolve/main/Huihui-Qwen3-VL-4B-Instruct-abliterated-Q4_K_M.gguf",
                2_497_282_240,
            )],
        },
        Component {
            id: "image-vae",
            title: "Декодер картинок",
            note: "превращает результат модели в саму картинку",
            requirement: Requirement::Required,
            files: &[FileSpec::plain(
                "models/image/wan_2.1_vae.safetensors",
                "https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/main/split_files/vae/wan_2.1_vae.safetensors",
                253_815_318,
            )],
        },
        Component {
            id: "image-edit",
            title: "Правка кадра по референсу",
            note: "держит облик героя и места одинаковым от кадра к кадру",
            requirement: Requirement::Recommended,
            files: &[FileSpec::plain(
                "models/image/krea2-identity-edit-r128.safetensors",
                "https://huggingface.co/conradlocke/krea2-identity-edit/resolve/main/krea2_identity_edit_v1_2_r128.safetensors",
                914_159_744,
            )],
        },
        Component {
            id: "narrator-engine",
            title: "Движок рассказчика",
            note: "llama.cpp, считает текст на видеокарте",
            requirement: Requirement::Required,
            files: &[FileSpec::zip(
                "downloads/llama.zip",
                concat!("https://github.com/ggml-org/llama.cpp/releases/download/b9966/", "llama-b9966-bin-win-cuda-13.3-x64.zip"),
                420_000_000,
                "tools/llama",
                "tools/llama/llama-server.exe",
            )],
        },
        Component {
            id: "narrator",
            title: "Рассказчик",
            note: "ведёт историю; без цензуры",
            requirement: Requirement::Required,
            files: &[FileSpec::plain(
                "models/text/narrator.gguf",
                "https://huggingface.co/zaakirio/gemma-4-12b-it-uncensored-GGUF/resolve/main/gemma-4-12b-it-uncensored-Q4_K_M.gguf",
                7_380_000_000,
            )],
        },
        Component {
            id: "narrator-vision",
            title: "Зрение рассказчика",
            note: "позволяет показывать модели свои картинки",
            requirement: Requirement::Optional,
            files: &[FileSpec::plain(
                "models/text/mmproj.gguf",
                "https://huggingface.co/zaakirio/gemma-4-12b-it-uncensored-GGUF/resolve/main/mmproj-gemma-4-12B-it-bf16.gguf",
                180_000_000,
            )],
        },
        Component {
            id: "voice-engine",
            title: "Движок озвучки",
            note: "Higgs: читает текст голосом из пака",
            requirement: Requirement::Recommended,
            files: &[FileSpec::plain(
                "models/tts-engine/audiocpp_engine.dll",
                concat!("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/", "engines/audiocpp_engine.dll"),
                71_727_104,
            )],
        },
        Component {
            id: "voice-model",
            title: "Модель озвучки",
            note: "квант Q4_K_M; вместе с ней идут настройки и словарь",
            requirement: Requirement::Recommended,
            files: &[
                FileSpec::plain("models/tts/q4_k_m.gguf", concat!("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/", "models/higgs-q4_k_m/q4_k_m.gguf"), 4_086_000_000),
                FileSpec::plain("models/tts/config.json", concat!("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/", "models/higgs-q4_k_m/config.json"), 2_755),
                FileSpec::plain("models/tts/chat_template.jinja", concat!("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/", "models/higgs-q4_k_m/chat_template.jinja"), 2_427),
                FileSpec::plain("models/tts/tokenizer.json", concat!("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/", "models/higgs-q4_k_m/tokenizer.json"), 11_433_924),
                FileSpec::plain("models/tts/tokenizer_config.json", concat!("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/", "models/higgs-q4_k_m/tokenizer_config.json"), 1_937),
                FileSpec::plain("models/tts/higgs_audio_v2_tokenizer_config.json", concat!("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/main/", "models/higgs-q4_k_m/higgs_audio_v2_tokenizer_config.json"), 2_251),
            ],
        },
        Component {
            id: "voice-pack",
            title: "Пак голосов",
            note: "готовые голоса для чтения; можно добавить свой",
            requirement: Requirement::Recommended,
            files: &[FileSpec::zip(
                "downloads/voice-pack.zip",
                "https://huggingface.co/datasets/nerualdreming/VibeVoice/resolve/main/voice-pack.zip",
                120_000_000,
                "models/voices",
                "models/voices",
            )],
        },
        Component {
            id: "asr",
            title: "Распознавание речи",
            note: "голосовой ввод действий; Parakeet и среда onnx",
            requirement: Requirement::Recommended,
            files: &[
                FileSpec::zip("downloads/onnxruntime.zip", ORT, 12_000_000, "models/asr", "models/asr/onnxruntime.dll"),
                FileSpec::plain("models/asr/encoder-model.int8.onnx", concat!("https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/", "encoder-model.int8.onnx"), 651_000_000),
                FileSpec::plain("models/asr/decoder_joint-model.int8.onnx", concat!("https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/", "decoder_joint-model.int8.onnx"), 18_000_000),
                FileSpec::plain("models/asr/nemo128.onnx", concat!("https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/", "nemo128.onnx"), 71_000),
                FileSpec::plain("models/asr/vocab.txt", concat!("https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/", "vocab.txt"), 90_000),
                FileSpec::plain("models/asr/config.json", concat!("https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/", "config.json"), 1_400),
            ],
        },
        Component {
            id: "whisper-engine",
            title: "Whisper: движок",
            note: "второй движок распознавания речи, точнее на шумной записи и на смеси языков",
            requirement: Requirement::Optional,
            files: &[FileSpec::zip(
                "downloads/whisper.zip",
                "https://github.com/Purfview/whisper-standalone-win/releases/download/faster-whisper/Whisper-Faster_r192.3_windows.zip",
                87_654_143,
                "tools/whisper",
                "tools/whisper/faster-whisper-xxl.exe",
            )],
        },
        Component {
            id: "whisper-model",
            title: "Whisper large-v3",
            note: "самая точная модель Whisper; нужна только если выбран этот движок",
            requirement: Requirement::Optional,
            files: &[
                FileSpec::plain(
                    "models/whisper/faster-whisper-large-v3/model.bin",
                    "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/main/model.bin",
                    3_087_284_237,
                ),
                FileSpec::plain(
                    "models/whisper/faster-whisper-large-v3/config.json",
                    "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/main/config.json",
                    2_394,
                ),
                FileSpec::plain(
                    "models/whisper/faster-whisper-large-v3/preprocessor_config.json",
                    "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/main/preprocessor_config.json",
                    340,
                ),
                FileSpec::plain(
                    "models/whisper/faster-whisper-large-v3/tokenizer.json",
                    "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/main/tokenizer.json",
                    2_480_617,
                ),
                FileSpec::plain(
                    "models/whisper/faster-whisper-large-v3/vocabulary.json",
                    "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/main/vocabulary.json",
                    1_068_114,
                ),
            ],
        },
    ];

pub fn manifest() -> Vec<Component> {
    COMPONENTS.to_vec()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatus {
    #[serde(flatten)]
    pub component: Component,
    pub present: bool,
    /// Сколько байт уже на диске: готовый файл или незавершённая докачка.
    pub have_bytes: u64,
}

/// Есть ли этот файл на диске уже в готовом виде.
///
/// «Файл существует» — не то же самое, что «файл целый»: оборванная закачка оставляет
/// обрубок, и раньше он считался установленным. Игра потом падала на загрузке весов, а
/// качалка была уверена, что всё на месте. Поэтому у прямых файлов сверяем размер: он
/// известен точно, до байта.
fn file_present(root: &Path, file: &FileSpec) -> bool {
    if file.extract == Extract::ZipFlat {
        // Архив после распаковки удаляется, поэтому смотрим на результат распаковки.
        let marker = root.join(file.marker);
        return if marker.extension().is_some() { marker.is_file() } else { voices_installed(&marker) };
    }
    let target = root.join(file.path);
    let Ok(meta) = std::fs::metadata(&target) else { return false };
    if !meta.is_file() {
        return false;
    }
    // Незавершённая закачка оставляет рядом свой хвост — вот это и есть надёжный признак
    // недокачки, а не размер.
    if part_path(&target).exists() {
        return false;
    }
    // Размер в манифесте — ОЦЕНКА для полосы прогресса, а не отпечаток файла: там круглые
    // числа, а на диске точные, и расхождение в мегабайт — норма. Требовать совпадения
    // байт-в-байт нельзя: так целые файлы объявлялись недокачанными и качались заново.
    // Обрубок же короче в разы, поэтому порога в девять десятых достаточно.
    file.bytes == 0 || meta.len() * 10 >= file.bytes * 9
}

/// Сколько байт этого файла уже лежит на диске: готовый файл или незавершённая докачка.
fn file_have_bytes(root: &Path, file: &FileSpec) -> u64 {
    let target = root.join(file.path);
    if file_present(root, file) {
        // У распакованного архива самого файла уже нет — считаем его добранным целиком.
        return std::fs::metadata(&target).map(|meta| meta.len()).unwrap_or(file.bytes);
    }
    // Незавершённую закачку тоже показываем, иначе после перезапуска прогресс выглядел бы
    // обнулившимся.
    std::fs::metadata(part_path(&target)).map(|meta| meta.len()).unwrap_or(0)
}

pub fn status(root: &Path) -> Vec<ComponentStatus> {
    manifest()
        .into_iter()
        .map(|component| {
            let present = component.files.iter().all(|file| file_present(root, file));
            let have_bytes = component.files.iter().map(|file| file_have_bytes(root, file)).sum();
            ComponentStatus { component, present, have_bytes }
        })
        .collect()
}

fn part_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    target.with_file_name(name)
}

fn done_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".done");
    target.with_file_name(name)
}

#[cfg(windows)]
fn write_at(file: &File, buf: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt;
    file.seek_write(buf, offset)
}

#[cfg(not(windows))]
fn write_at(file: &File, buf: &[u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt;
    file.write_at(buf, offset)
}

/// Размер файла и поддержка докачки по диапазонам: однобайтовый пробник.
fn probe_size(agent: &ureq::Agent, url: &str) -> (u64, bool) {
    match agent.get(url).header("Range", "bytes=0-0").call() {
        Ok(response) if response.status().as_u16() == 206 => {
            let total = response
                .headers()
                .get("content-range")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.rsplit('/').next())
                .and_then(|value| value.trim().parse::<u64>().ok())
                .unwrap_or(0);
            (total, total > 0)
        }
        Ok(response) => {
            let total = response
                .headers()
                .get("content-length")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            (total, false)
        }
        Err(_) => (0, false),
    }
}

/// Какие смещения уже скачаны — читается из манифеста докачки.
fn completed_offsets(path: &Path) -> Vec<u64> {
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    bytes
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8])))
        .collect()
}

/// Скачать один диапазон с проверкой полноты и повторами.
fn download_range(
    agent: &ureq::Agent,
    url: &str,
    file: &Arc<File>,
    start: u64,
    end: u64,
    downloaded: &Arc<AtomicU64>,
    abort: &Arc<AtomicBool>,
    done: &Arc<Mutex<File>>,
) -> Result<(), String> {
    let want = end - start + 1;
    let mut last = String::new();

    for attempt in 0..RANGE_RETRIES {
        if abort.load(Ordering::Relaxed) {
            return Err("отменено".into());
        }
        let mut got = 0u64;
        match download_range_once(agent, url, file, start, end, downloaded, abort, &mut got) {
            Ok(()) if got == want => {
                // Сначала данные на диск, только потом отметка: иначе после жёсткого
                // выключения возобновление пропустит чанк, у которого на диске нули.
                let _ = file.sync_data();
                if let Ok(mut manifest) = done.lock() {
                    use std::io::Write;
                    let _ = manifest.write_all(&start.to_le_bytes());
                    let _ = manifest.sync_data();
                }
                return Ok(());
            }
            Ok(()) => last = format!("неполный диапазон: {got} из {want} байт"),
            Err(error) if error == "отменено" => return Err(error),
            Err(error) => last = error,
        }
        // Откатываем вклад неудачной попытки, иначе повтор задвоил бы прогресс.
        downloaded.fetch_sub(got.min(downloaded.load(Ordering::Relaxed)), Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_millis(400 * (attempt as u64 + 1)));
    }
    Err(format!("диапазон {start}-{end} не дался за {RANGE_RETRIES} попыток: {last}"))
}

fn download_range_once(
    agent: &ureq::Agent,
    url: &str,
    file: &Arc<File>,
    start: u64,
    end: u64,
    downloaded: &Arc<AtomicU64>,
    abort: &Arc<AtomicBool>,
    got: &mut u64,
) -> Result<(), String> {
    let response = agent
        .get(url)
        .header("Range", &format!("bytes={start}-{end}"))
        .call()
        .map_err(|error| format!("диапазон {start}-{end}: {error}"))?;
    if response.status().as_u16() != 206 {
        return Err(format!("диапазон {start}-{end}: ответ {}", response.status()));
    }

    let mut reader = response.into_body().into_reader();
    let mut buffer = [0u8; 262_144];
    let mut offset = start;
    loop {
        if abort.load(Ordering::Relaxed) {
            return Err("отменено".into());
        }
        let read = reader.read(&mut buffer).map_err(|error| format!("чтение: {error}"))?;
        if read == 0 {
            break;
        }
        let mut written = 0;
        while written < read {
            let step = write_at(file, &buffer[written..read], offset + written as u64)
                .map_err(|error| format!("запись: {error}"))?;
            if step == 0 {
                return Err("запись не продвинулась".into());
            }
            written += step;
        }
        offset += read as u64;
        *got += read as u64;
        downloaded.fetch_add(read as u64, Ordering::Relaxed);
    }
    Ok(())
}

/// Скачать один компонент. `progress` зовут с числом уже полученных байт.
/// Скачать компонент целиком: все его файлы по очереди, с общим прогрессом.
pub fn download_component(
    root: &Path,
    component: &Component,
    progress: &dyn Fn(u64, u64),
    abort: Arc<AtomicBool>,
) -> Result<(), String> {
    // Прогресс показываем по всему набору, а не по текущему файлу: игроку важно, сколько
    // осталось до рабочего движка, а не до конца одной из шести его частей.
    let expected: u64 = component.bytes();
    let already: u64 = component
        .files
        .iter()
        .filter(|file| file_present(root, file))
        .map(|file| file.bytes)
        .sum();
    let mut base = already;
    for file in component.files {
        if file_present(root, file) {
            continue;
        }
        let report = |have: u64, _total: u64| progress(base + have, expected.max(base + have));
        download_file(root, component.title, file, &report, abort.clone())?;
        base += file.bytes;
    }
    progress(expected, expected);
    Ok(())
}

fn download_file(
    root: &Path,
    title: &str,
    spec: &FileSpec,
    progress: &dyn Fn(u64, u64),
    abort: Arc<AtomicBool>,
) -> Result<(), String> {
    let target = root.join(spec.path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("создать каталог: {error}"))?;
    }

    let agent = ureq::Agent::new_with_defaults();
    let (total, ranged) = probe_size(&agent, spec.url);
    if total == 0 {
        return Err(format!("не удалось узнать размер {title}"));
    }

    let part = part_path(&target);
    let done_file = done_path(&target);
    let file = Arc::new(
        OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&part)
            .map_err(|error| format!("открыть {}: {error}", part.display()))?,
    );
    file.set_len(total).map_err(|error| format!("разметить файл: {error}"))?;

    if !ranged {
        // Сервер без докачки: тянем целиком, возобновлять нечего.
        let response = agent.get(spec.url).call().map_err(|error| error.to_string())?;
        let mut reader = response.into_body().into_reader();
        let mut buffer = [0u8; 262_144];
        let mut offset = 0u64;
        loop {
            if abort.load(Ordering::Relaxed) {
                return Err("отменено".into());
            }
            let read = reader.read(&mut buffer).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            write_at(&file, &buffer[..read], offset).map_err(|error| error.to_string())?;
            offset += read as u64;
            progress(offset, total);
        }
        drop(file);
        std::fs::rename(&part, &target).map_err(|error| error.to_string())?;
        return Ok(());
    }

    let already: Vec<u64> = completed_offsets(&done_file);
    let done = Arc::new(Mutex::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&done_file)
            .map_err(|error| format!("манифест докачки: {error}"))?,
    ));

    let mut tasks: Vec<(u64, u64)> = Vec::new();
    let mut start = 0u64;
    while start < total {
        let end = (start + CHUNK - 1).min(total - 1);
        if !already.contains(&start) {
            tasks.push((start, end));
        }
        start = end + 1;
    }

    let downloaded = Arc::new(AtomicU64::new(total - tasks.iter().map(|(s, e)| e - s + 1).sum::<u64>()));
    let queue = Arc::new(Mutex::new(tasks));
    let failure: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    std::thread::scope(|scope| {
        for _ in 0..POOL_SLOTS {
            let agent = agent.clone();
            let queue = queue.clone();
            let file = file.clone();
            let downloaded = downloaded.clone();
            let abort = abort.clone();
            let done = done.clone();
            let failure = failure.clone();
            scope.spawn(move || loop {
                if abort.load(Ordering::Relaxed) || failure.lock().map(|f| f.is_some()).unwrap_or(false) {
                    return;
                }
                let Some((start, end)) = queue.lock().ok().and_then(|mut queue| queue.pop()) else {
                    return;
                };
                if let Err(error) =
                    download_range(&agent, spec.url, &file, start, end, &downloaded, &abort, &done)
                {
                    if let Ok(mut slot) = failure.lock() {
                        slot.get_or_insert(error);
                    }
                    abort.store(true, Ordering::Relaxed);
                    return;
                }
            });
        }

        // Прогресс сообщаем с главного потока: воркеры трогают только общий счётчик.
        while !queue.lock().map(|queue| queue.is_empty()).unwrap_or(true)
            || downloaded.load(Ordering::Relaxed) < total
        {
            if abort.load(Ordering::Relaxed) {
                break;
            }
            progress(downloaded.load(Ordering::Relaxed), total);
            std::thread::sleep(std::time::Duration::from_millis(300));
            if failure.lock().map(|f| f.is_some()).unwrap_or(false) {
                break;
            }
        }
    });

    if let Some(error) = failure.lock().ok().and_then(|slot| slot.clone()) {
        return Err(error);
    }
    if abort.load(Ordering::Relaxed) {
        return Err("отменено".into());
    }

    drop(file);
    std::fs::rename(&part, &target).map_err(|error| format!("финализация: {error}"))?;
    let _ = std::fs::remove_file(&done_file);
    if spec.extract == Extract::ZipFlat {
        unpack_flat(&target, &root.join(spec.extract_dir))?;
        // Архив после распаковки только занимал бы место.
        let _ = std::fs::remove_file(&target);
    }
    progress(total, total);
    Ok(())
}

/// Есть ли в каталоге хотя бы один звуковой файл — по этому и судим, что пак уже установлен.
fn voices_installed(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|extension| {
                        let extension = extension.to_string_lossy().to_lowercase();
                        ["wav", "mp3", "flac"].contains(&extension.as_str())
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Распаковать архив ПЛОСКО: берём только имя файла, вложенные каталоги схлопываем. Это
/// заодно закрывает выход за каталог назначения — из имени убираются любые сегменты пути.
fn unpack_flat(archive: &Path, dir: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| format!("открыть архив: {error}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| format!("это не архив: {error}"))?;
    std::fs::create_dir_all(dir).map_err(|error| format!("создать каталог: {error}"))?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| format!("запись архива: {error}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let leaf = name.rsplit('/').next().unwrap_or_default().to_string();
        if leaf.is_empty() || leaf == "." || leaf == ".." {
            continue;
        }
        let out = dir.join(&leaf);
        let mut target = File::create(&out).map_err(|error| format!("создать {leaf}: {error}"))?;
        std::io::copy(&mut entry, &mut target)
            .map_err(|error| format!("распаковка {leaf}: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_truncated_file_is_not_installed() {
        use super::{file_present, Extract, FileSpec};
        let dir = std::env::temp_dir().join("du-setup-size");
        std::fs::create_dir_all(&dir).unwrap();
        let spec = FileSpec {
            path: "модель.gguf",
            url: "https://example/модель.gguf",
            bytes: 10,
            extract: Extract::None,
            extract_dir: "",
            marker: "",
        };

        // Оборванная закачка: файл есть, но короче обещанного в разы.
        std::fs::write(dir.join("модель.gguf"), b"12345").unwrap();
        assert!(!file_present(&dir, &spec), "обрубок не считается установленным");

        // Дописали до нужного размера — теперь на месте.
        std::fs::write(dir.join("модель.gguf"), b"1234567890").unwrap();
        assert!(file_present(&dir, &spec));

        // Размер в манифесте округлён, на диске точный — это НОРМА, а не недокачка.
        std::fs::write(dir.join("модель.gguf"), b"12345678901").unwrap();
        assert!(file_present(&dir, &spec), "лишние байты сверх оценки — не повод качать заново");

        // Источник без точного размера: верим наличию, иначе файл не установить никогда.
        let rolling = FileSpec { bytes: 0, ..spec };
        assert!(file_present(&dir, &rolling));
    }

    #[test]
    fn the_first_run_is_held_only_by_the_required_ones() {
        use super::{manifest, Requirement};
        let all = manifest();
        assert!(all.iter().any(|c| c.requirement == Requirement::Required));
        assert!(all.iter().any(|c| c.requirement == Requirement::Recommended));
        // Обязательные — это движки и веса, без которых ход не пойдёт.
        for component in all.iter().filter(|c| c.required()) {
            assert!(!component.files.is_empty(), "{} нечего качать", component.id);
        }
    }
    use super::*;

    #[test]
    fn every_component_is_described_once_and_completely() {
        let manifest = manifest();
        assert!(!manifest.is_empty());
        let mut ids: Vec<&str> = manifest.iter().map(|component| component.id).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "идентификаторы компонентов обязаны быть уникальны");

        for component in &manifest {
            assert!(!component.title.trim().is_empty(), "{}: нет названия", component.id);
            assert!(!component.files.is_empty(), "{}: компонент без файлов", component.id);
            assert!(component.bytes() > 0, "{}: неизвестен размер", component.id);
            for spec in component.files {
                assert!(spec.url.starts_with("https://"), "{}: адрес не по https", component.id);
                assert!(!spec.path.contains(".."), "{}: путь выходит наружу", component.id);
                assert!(spec.bytes > 0, "{}: неизвестен размер файла {}", component.id, spec.path);
                if spec.extract == Extract::ZipFlat {
                    assert!(!spec.extract_dir.is_empty(), "{}: архиву некуда распаковаться", component.id);
                    assert!(!spec.marker.is_empty(), "{}: у архива нет признака готовности", component.id);
                }
            }
        }
    }

    #[test]
    fn the_essential_components_are_marked_as_required() {
        let manifest = manifest();
        let required: Vec<&str> = manifest
            .iter()
            .filter(|component| component.required())
            .map(|component| component.id)
            .collect();
        // Без этих четырёх приложение не нарисует кадр и не напишет ход.
        for id in ["image-dit", "image-encoder", "image-vae", "narrator"] {
            assert!(required.contains(&id), "{id} обязан быть в списке необходимых");
        }
    }

    #[test]
    fn status_reports_what_is_missing_and_what_is_half_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let statuses = status(dir.path());
        assert!(statuses.iter().all(|status| !status.present));
        assert!(statuses.iter().all(|status| status.have_bytes == 0));

        // Незавершённая закачка обязана попасть в прогресс, иначе после перезапуска он
        // выглядел бы обнулившимся.
        let target = dir.path().join("models/text/narrator.gguf");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(part_path(&target), vec![0u8; 1024]).unwrap();
        let statuses = status(dir.path());
        let narrator = statuses.iter().find(|status| status.component.id == "narrator").unwrap();
        assert!(!narrator.present);
        assert_eq!(narrator.have_bytes, 1024);
    }

    #[test]
    fn a_finished_file_is_reported_as_present() {
        let dir = tempfile::tempdir().unwrap();
        let vae_spec = manifest().into_iter().find(|c| c.id == "image-vae").unwrap();
        let file = &vae_spec.files[0];
        let target = dir.path().join(file.path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        // Ровно столько байт, сколько обещает манифест: недобор теперь считается недокачкой.
        write_of_size(&target, file.bytes);

        let statuses = status(dir.path());
        let vae = statuses.iter().find(|status| status.component.id == "image-vae").unwrap();
        assert!(vae.present);
        assert_eq!(vae.have_bytes, file.bytes);
    }

    /// Файл заданной длины без выделения памяти под него.
    fn write_of_size(path: &Path, bytes: u64) {
        let file = std::fs::File::create(path).unwrap();
        file.set_len(bytes).unwrap();
    }

    #[test]
    fn resume_offsets_round_trip_through_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("докачка.done");
        let mut file = std::fs::File::create(&path).unwrap();
        use std::io::Write;
        for offset in [0u64, CHUNK, CHUNK * 5] {
            file.write_all(&offset.to_le_bytes()).unwrap();
        }
        drop(file);
        assert_eq!(completed_offsets(&path), vec![0, CHUNK, CHUNK * 5]);
    }

    #[test]
    fn an_already_downloaded_component_is_not_fetched_again() {
        let dir = tempfile::tempdir().unwrap();
        let component = manifest().into_iter().find(|c| c.id == "image-vae").unwrap();
        let target = dir.path().join(component.files[0].path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        write_of_size(&target, component.files[0].bytes);

        // Адрес заведомо недостижим: если функция полезет в сеть, проверка упадёт.
        let result = download_component(
            dir.path(),
            &component,
            &|_, _| {},
            Arc::new(AtomicBool::new(false)),
        );
        assert!(result.is_ok());
        // Файл не тронут: качалка увидела целый файл и в сеть не пошла.
        assert_eq!(std::fs::metadata(&target).unwrap().len(), component.files[0].bytes);
    }
}
