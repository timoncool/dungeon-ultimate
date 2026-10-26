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
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// Одновременных соединений на всю закачку.
const POOL_SLOTS: usize = 4;
/// Размер одной задачи: делит крупный файл на части и не даёт мелким ждать больших.
const CHUNK: u64 = 16 * 1024 * 1024;
/// Сколько раз повторяем ОДИН диапазон, прежде чем признать закачку неудачной.
const RANGE_RETRIES: u32 = 8;
/// Попыток до хоста, с которым не открывается соединение (нет DNS, маршрута, отказ, таймаут подключения), прежде
/// чем сказать об этом: каждая стоит до таймаута подключения, а закрытый хост так и остаётся закрытым — восемь
/// таких держали игрока перед стоящей шкалой минутами.
const CONNECT_RETRIES: u32 = 3;

/// Что делать с файлом после закачки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Extract {
    /// Ничего: файл кладётся как есть.
    None,
    /// Zip распаковывается ПЛОСКО в каталог назначения: паки приходят и с папкой внутри,
    /// и без неё, а раскладка должна получаться одинаковая.
    ZipFlat,
    /// Zip распаковывается с сохранением поддерева.
    ZipTree,
    /// Из zip берутся только библиотеки `.dll`, плоско: архивы NVIDIA несут ещё заголовки и
    /// `.lib`, которые рядом с программой не нужны.
    ZipDlls,
}

/// Один скачиваемый файл компонента.
#[derive(Debug, Clone, Serialize)]
pub struct FileSpec {
    /// Куда положить относительно корня приложения.
    pub path: &'static str,
    /// Адрес с закреплённой ревизией: у Hugging Face — коммит, у GitHub — тег релиза.
    pub url: &'static str,
    /// Точный размер в байтах.
    pub bytes: u64,
    /// SHA-256 файла (у архива — самого архива). Не совпал — файл удаляется, закачка падает.
    pub sha256: &'static str,
    /// Что сделать после закачки.
    pub extract: Extract,
    /// Куда распаковывать архив.
    pub extract_dir: &'static str,
    /// По какому файлу или каталогу считать распакованное установленным.
    pub marker: &'static str,
}

impl FileSpec {
    /// Обычный файл: положить как есть.
    const fn plain(path: &'static str, url: &'static str, bytes: u64, sha256: &'static str) -> Self {
        Self { path, url, bytes, sha256, extract: Extract::None, extract_dir: "", marker: "" }
    }

    /// Архив: распаковать в каталог и считать установленным по маркеру.
    const fn archive(
        path: &'static str,
        url: &'static str,
        bytes: u64,
        sha256: &'static str,
        extract: Extract,
        dir: &'static str,
        marker: &'static str,
    ) -> Self {
        Self { path, url, bytes, sha256, extract, extract_dir: dir, marker }
    }

    fn is_archive(&self) -> bool {
        self.extract != Extract::None
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
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: &'static str,
    pub title: &'static str,
    /// Зачем оно нужно — показывается под названием.
    pub note: &'static str,
    /// Насколько нужен для игры.
    pub requirement: Requirement,
    /// Группа взаимозаменяемых вариантов одной стадии (`asr`); пусто — компонент сам по себе.
    /// Скачанный вариант закрывает группу, и вариант по умолчанию больше не числится недостающим.
    pub variant_of: &'static str,
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
// Набор и версии — те же, что в Dub Studio. У каждого файла закреплена ревизия (коммит Hugging Face
// или тег релиза GitHub) и SHA-256: перевыложенный апстримом файл не подменит веса молча.
// Движок картинок обязан совпадать с `du_image::PINNED_COMMIT`, иначе загрузчик откажется работать
// с чужой сборкой. Адреса стоят целиком, потому что срез файлов статический.

/// Рантайм CUDA 13 и cuBLAS 13 из колёс NVIDIA: их импортируют движок озвучки и сборка transcribe.cpp под
/// видеокарту, путь один, поэтому файлы качаются один раз на оба компонента.
const CUDART13: FileSpec = FileSpec::archive("downloads/tts-cudart.whl", "https://files.pythonhosted.org/packages/86/00/d5436004268f049214193659ebc36550b5ef3925c3d13b4cc980e13be6f5/nvidia_cuda_runtime-13.4.92-py3-none-win_amd64.whl", 2_778_543, "08dca5e4aba480c2fd5b55075c0fa71b84ef9dcf0521f2d58baa14a803a7311c", Extract::ZipDlls, "models/tts-engine", "models/tts-engine/cudart64_13.dll");
// cuFFT 12 — третья библиотека CUDA 13, которую импортирует audiocpp_server (cudart в нём слинкован статически)
const CUFFT12: FileSpec = FileSpec::archive("downloads/tts-cufft.zip", "https://developer.download.nvidia.com/compute/cuda/redist/libcufft/windows-x86_64/libcufft-windows-x86_64-12.4.0.43-archive.zip", 159_602_693, "69d0ad8dc3a1be66f01a748a8206d0ceaafa56939474663fbe790dc3e91d2009", Extract::ZipDlls, "models/tts-engine", "models/tts-engine/cufft64_12.dll");

/// Сборка audiocpp_server (голос Higgs и CrisperWhisper) — архив релиза, закреплённый в крейте `acpp-server`.
const fn audiocpp(asset: acpp_server::asset::Asset, download: &'static str, dir: &'static str, marker: &'static str) -> FileSpec {
    FileSpec::archive(download, asset.url, asset.size, asset.sha256, Extract::ZipFlat, dir, marker)
}
const AUDIOCPP_CUDA13: FileSpec = audiocpp(acpp_server::asset::SERVER_WIN_CUDA13, "downloads/audiocpp.zip", "tools/audiocpp", "tools/audiocpp/audiocpp_server.exe");
const AUDIOCPP_CUDA12: FileSpec = audiocpp(acpp_server::asset::SERVER_WIN_CUDA12, "downloads/audiocpp-cuda12.zip", "tools/audiocpp-cuda12", "tools/audiocpp-cuda12/audiocpp_server.exe");
const AUDIOCPP_VULKAN: FileSpec = audiocpp(acpp_server::asset::SERVER_WIN_VULKAN, "downloads/audiocpp-vulkan.zip", "tools/audiocpp-vulkan", "tools/audiocpp-vulkan/audiocpp_server.exe");
const AUDIOCPP_CPU: FileSpec = audiocpp(acpp_server::asset::SERVER_WIN_CPU, "downloads/audiocpp-cpu.zip", "tools/audiocpp-cpu", "tools/audiocpp-cpu/audiocpp_server.exe");

/// CUDA 12.9 для сборки сервера под CUDA 12 (колёса PyPI, их `.dll` раскладываются плоско) — те же файлы и папка, что у Dub.
const fn cuda12_runtime(asset: acpp_server::asset::Asset, download: &'static str, marker: &'static str) -> FileSpec {
    FileSpec::archive(download, asset.url, asset.size, asset.sha256, Extract::ZipDlls, "models/cuda12-runtime", marker)
}
const CUDA12_CUDART: FileSpec = cuda12_runtime(acpp_server::asset::CUDA12_RUNTIME[0], "downloads/cuda12-cudart.whl", "models/cuda12-runtime/cudart64_12.dll");
const CUDA12_CUBLAS: FileSpec = cuda12_runtime(acpp_server::asset::CUDA12_RUNTIME[1], "downloads/cuda12-cublas.whl", "models/cuda12-runtime/cublas64_12.dll");
const CUDA12_CUFFT: FileSpec = cuda12_runtime(acpp_server::asset::CUDA12_RUNTIME[2], "downloads/cuda12-cufft.whl", "models/cuda12-runtime/cufft64_11.dll");

const CUBLAS13: FileSpec = FileSpec::archive("downloads/tts-cublas.whl", "https://files.pythonhosted.org/packages/a3/df/f1246959833e2c437db8be3e5b477f66b87f8817821ed40de6c7561c9a36/nvidia_cublas-13.8.0.4-py3-none-win_amd64.whl", 423_266_897, "8c5494423bb8a46822cb6b0cb95d7fa4be2d7b96a31155dff083839ec8297910", Extract::ZipDlls, "models/tts-engine", "models/tts-engine/cublas64_13.dll");

/// Сборки transcribe.cpp (видеокарта и процессор) в папках и с файлами Dub Studio: процессорная идёт с моделью,
/// видеокарточная — отдельным компонентом `asr-gpu`.
const TCPP_CUDA: FileSpec = tcpp_engine(dub_tcpp::Build::Cuda, "models/transcribe-engine/transcribe_engine.dll");
const TCPP_CPU: FileSpec = tcpp_engine(dub_tcpp::Build::Cpu, "models/transcribe-engine-cpu/transcribe_engine.dll");

const fn tcpp_engine(build: dub_tcpp::Build, path: &'static str) -> FileSpec {
    let download = build.download();
    FileSpec::plain(path, download.url, download.size, download.sha256)
}

/// Модель Parakeet на transcribe.cpp по пути Dub Studio (`models/<dir>/<file>`).
const fn parakeet_gguf(model: &'static dub_tcpp::Model, path: &'static str) -> FileSpec {
    FileSpec::plain(path, model.download.url, model.download.size, model.download.sha256)
}

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
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/sd-engine.zip", "https://github.com/leejet/stable-diffusion.cpp/releases/download/master-956-1b0ba10/sd-master-1b0ba10-bin-win-cuda12-x64.zip", 337_974_452, "144b9631bf2eeb9bf6bf7f148cf40aaf05739ce7c49d3ac0ec2ddf6b65765d69", Extract::ZipFlat, "models/runtime/sd", "models/runtime/sd/stable-diffusion.dll"),
                FileSpec::archive("downloads/sd-cuda.zip", "https://github.com/leejet/stable-diffusion.cpp/releases/download/master-956-1b0ba10/cudart-sd-bin-win-cu12-x64.zip", 563_452_046, "fe20366827d357c00797eebb58244dddab7fd9a348d70090c3871004c320f38d", Extract::ZipFlat, "models/runtime/sd", "models/runtime/sd/cudart64_12.dll"),
            ],
        },
        Component {
            id: "image-engine-vulkan",
            title: "Движок картинок под Vulkan",
            note: "stable-diffusion.cpp для видеокарт AMD и Intel",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/sd-engine-vulkan.zip", "https://github.com/leejet/stable-diffusion.cpp/releases/download/master-956-1b0ba10/sd-master-1b0ba10-bin-win-vulkan-x64.zip", 30_123_908, "9e8b9baff514fe5dfb77e51bf08d85f11e6f10128eac14d4474c87f563309725", Extract::ZipFlat, "models/runtime/sd-vulkan", "models/runtime/sd-vulkan/stable-diffusion.dll"),
            ],
        },
        Component {
            id: "image-dit",
            title: "Модель картинок Krea-2 Turbo",
            note: "рисует кадры сцены; квант Q4_K_M",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::plain("models/image/krea2-turbo-Q4_K_M.gguf", "https://huggingface.co/realrebelai/KREA-2_GGUFs/resolve/9721c3ec932c43dff6dbb5793a816d9ad105277f/TURBO/Krea-2-Turbo-Q4_K_M.gguf", 7_216_993_376, "273a98be1afe317bc7228403b6434647eaf866cebe6aff1980c401b950473807"),
            ],
        },
        Component {
            id: "image-encoder",
            title: "Текстовый энкодер картинок",
            note: "переводит промпт в понятное модели; без цензуры",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::plain("models/image/qwen3-vl-4b-abliterated-Q4_K_M.gguf", "https://huggingface.co/noctrex/Huihui-Qwen3-VL-4B-Instruct-abliterated-GGUF/resolve/0e204140ef43727fb4ee4d4bbd88c948c64719df/Huihui-Qwen3-VL-4B-Instruct-abliterated-Q4_K_M.gguf", 2_497_282_240, "9789bbc02e77bf3b9cfe7b54a6b212a1e8904ffc77df61243353a05fcb203167"),
            ],
        },
        Component {
            id: "image-vae",
            title: "Декодер картинок",
            note: "превращает результат модели в саму картинку",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::plain("models/image/wan_2.1_vae.safetensors", "https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/123acf1cc74bccbb9bfff8ac1ee72edc08c2341d/split_files/vae/wan_2.1_vae.safetensors", 253_815_318, "2fc39d31359a4b0a64f55876d8ff7fa8d780956ae2cb13463b0223e15148976b"),
            ],
        },
        Component {
            id: "image-edit",
            title: "Правка кадра по референсу",
            note: "держит облик героя и места одинаковым от кадра к кадру",
            requirement: Requirement::Recommended,
            variant_of: "",
            files: &[
                FileSpec::plain("models/image/krea2-identity-edit-r128.safetensors", "https://huggingface.co/conradlocke/krea2-identity-edit/resolve/89e9e7a09ee2e5c9331e952063d79b1b8a703280/krea2_identity_edit_v1_2_r128.safetensors", 914_159_744, "f53db0bb4b081d638f196865cbc9f055379704fafb788336784fc1ccde18d825"),
            ],
        },
        Component {
            id: "narrator-engine",
            title: "Движок рассказчика",
            note: "llama.cpp, считает текст на видеокарте, с библиотеками CUDA 13",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/llama.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11900/llama-b11900-bin-win-cuda-13.4-x64.zip", 164_721_212, "0d75ad36367da00622c146a97e82fac3dcfcbe960afc2f0234eef026886b16f6", Extract::ZipFlat, "tools/llama", "tools/llama/llama-server.exe"),
                FileSpec::archive("downloads/llama-cuda.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11900/cudart-llama-bin-win-cuda-13.4-x64.zip", 423_535_356, "738f8c251ac22b70c3ae6f83a10cf222725df0395246a2cf58f32bdb85fbe668", Extract::ZipFlat, "tools/llama", "tools/llama/cudart64_13.dll"),
            ],
        },
        Component {
            id: "narrator-engine-cuda12",
            title: "Движок рассказчика для старых карт NVIDIA",
            note: "llama.cpp с CUDA 12: Maxwell, Pascal, Volta и карты со старым драйвером",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/llama-cuda12.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11900/llama-b11900-bin-win-cuda-12.4-x64.zip", 279_417_051, "387ad76c926cdb301971851a11df018165a7b4df4b1b7c64cf4b09a367ec82bc", Extract::ZipFlat, "tools/llama-cuda12", "tools/llama-cuda12/llama-server.exe"),
                FileSpec::archive("downloads/llama-cuda12-runtime.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11900/cudart-llama-bin-win-cuda-12.4-x64.zip", 391_443_627, "8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6", Extract::ZipFlat, "tools/llama-cuda12", "tools/llama-cuda12/cudart64_12.dll"),
            ],
        },
        Component {
            id: "narrator-engine-vulkan",
            title: "Движок рассказчика под Vulkan",
            note: "llama.cpp для видеокарт AMD и Intel",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/llama-vulkan.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11900/llama-b11900-bin-win-vulkan-x64.zip", 37_051_563, "baf0183b1906d7cc0cfe9a0bd3fae34ceb4048acecc371ff3b7a91456783d051", Extract::ZipFlat, "tools/llama-vulkan", "tools/llama-vulkan/llama-server.exe"),
            ],
        },
        Component {
            id: "narrator-engine-cpu",
            title: "Движок рассказчика на процессоре",
            note: "llama.cpp для компьютера без видеокарты",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/llama-cpu.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11900/llama-b11900-bin-win-cpu-x64.zip", 24_228_419, "d3bbbbb7d1b4979b72052fd73cdeeb5a4d5a4e6a3f52aeb40abd6f116558e323", Extract::ZipFlat, "tools/llama-cpu", "tools/llama-cpu/llama-server.exe"),
            ],
        },
        Component {
            id: "narrator",
            title: "Рассказчик",
            note: "ведёт историю; без цензуры",
            requirement: Requirement::Required,
            variant_of: "",
            files: &[
                FileSpec::plain("models/text/narrator.gguf", "https://huggingface.co/zaakirio/gemma-4-12b-it-uncensored-GGUF/resolve/32880562ac43cb589a85afb864309fdcaf486fae/gemma-4-12b-it-uncensored-Q4_K_M.gguf", 7_381_381_760, "b2e1db5f81b9f0ce489f99fd448bbf4ea8ea719e8d28c05aaa5774afc5def76c"),
            ],
        },
        Component {
            id: "voice-engine",
            title: "Движок озвучки",
            note: "audiocpp_server под CUDA 13: Higgs читает текст голосом из пака, он же распознаёт CrisperWhisper",
            requirement: Requirement::Recommended,
            variant_of: "",
            // программа импортирует cuBLAS 13 и cuFFT 12: без них Windows её не запустит
            files: &[AUDIOCPP_CUDA13, CUBLAS13, CUFFT12],
        },
        Component {
            id: "voice-engine-cuda12",
            title: "Движок озвучки",
            note: "audiocpp_server под CUDA 12.9 с её библиотеками — для карт и драйверов, которым CUDA 13 не подходит",
            requirement: Requirement::Recommended,
            variant_of: "",
            files: &[AUDIOCPP_CUDA12, CUDA12_CUDART, CUDA12_CUBLAS, CUDA12_CUFFT],
        },
        Component {
            id: "voice-engine-vulkan",
            title: "Движок озвучки",
            note: "audiocpp_server под Vulkan — для видеокарт AMD и Intel",
            requirement: Requirement::Recommended,
            variant_of: "",
            files: &[AUDIOCPP_VULKAN],
        },
        Component {
            id: "voice-engine-cpu",
            title: "Движок озвучки",
            note: "audiocpp_server на процессоре — для компьютера без видеокарты",
            requirement: Requirement::Recommended,
            variant_of: "",
            files: &[AUDIOCPP_CPU],
        },
        Component {
            id: "voice-model",
            title: "Модель озвучки",
            note: "квант Q4_K_M; вместе с ней идут настройки и словарь",
            requirement: Requirement::Recommended,
            variant_of: "",
            files: &[
                FileSpec::plain("models/tts/q4_k_m.gguf", "https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/c6e9db5a2062c15accc1b9bfa54d927bbdb124dc/models/higgs-q4_k_m/q4_k_m.gguf", 4_086_922_976, "a6c8a9b5c8c72965865988c6ef411d32446aeb0df730f59e7bd4a3e801cea3a1"),
                FileSpec::plain("models/tts/config.json", "https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/c6e9db5a2062c15accc1b9bfa54d927bbdb124dc/models/higgs-q4_k_m/config.json", 2_755, "2ead4442c079ee35c2123a5b197e126e18eccfc0bdb65d31c94767e75d7864d4"),
                FileSpec::plain("models/tts/chat_template.jinja", "https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/c6e9db5a2062c15accc1b9bfa54d927bbdb124dc/models/higgs-q4_k_m/chat_template.jinja", 2_427, "44d5f08f3f72b837eaad09f13a54c1f9f4eb58d75240334548b7fd52a5437fa5"),
                FileSpec::plain("models/tts/tokenizer.json", "https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/c6e9db5a2062c15accc1b9bfa54d927bbdb124dc/models/higgs-q4_k_m/tokenizer.json", 11_433_924, "eb883de2de5adc5113f1f02b54830a0ea7cd6ef191cde65c41aceb3737d4d1c1"),
                FileSpec::plain("models/tts/tokenizer_config.json", "https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/c6e9db5a2062c15accc1b9bfa54d927bbdb124dc/models/higgs-q4_k_m/tokenizer_config.json", 1_937, "b4d632e1239569fb1829bf0bfa3c674fa54f22c42e9cb2669d77c438d4b4e02c"),
                FileSpec::plain("models/tts/higgs_audio_v2_tokenizer_config.json", "https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/c6e9db5a2062c15accc1b9bfa54d927bbdb124dc/models/higgs-q4_k_m/higgs_audio_v2_tokenizer_config.json", 2_251, "1f96f10516c2bb59d5a127e04e659a331a50fb1684217b1492fabd1dc94def26"),
            ],
        },
        Component {
            id: "voice-pack",
            title: "Пак голосов",
            note: "готовые голоса для чтения; можно добавить свой",
            requirement: Requirement::Recommended,
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/voice-pack.zip", "https://huggingface.co/datasets/nerualdreming/VibeVoice/resolve/5c884b3cbdc932c39a5167951c4072120e1a0310/voice-pack.zip", 104_685_334, "682e73c189dbe9bc66fc35bc8fb29438641155957a917a7cc914f2f176a65264", Extract::ZipFlat, "models/voices", "models/voices"),
            ],
        },
        Component {
            id: "asr",
            title: "Распознавание речи: Parakeet",
            note: "голосовой ввод действий; 25 европейских языков, быстрый",
            requirement: Requirement::Recommended,
            variant_of: "asr",
            files: &[
                parakeet_gguf(&dub_tcpp::MODELS[0], "models/asr-gguf-parakeet-v3/parakeet-tdt-0.6b-v3-Q8_0.gguf"),
                TCPP_CPU,
            ],
        },
        Component {
            id: "asr-ultra",
            title: "Распознавание речи: Parakeet Ultra",
            note: "та же модель, дообученная Moondream: точнее на тех же 25 языках",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                parakeet_gguf(&dub_tcpp::MODELS[1], "models/asr-gguf-parakeet-ultra/parakeet-ultra-Q8_0.gguf"),
                TCPP_CPU,
            ],
        },
        // CrisperWhisper 2.0 turbo (nyralabs, лицензия nyra health Non-Commercial, LICENSE.md рядом): репозиторий Hugging
        // Face как есть, ревизия закреплена, папка Dub Studio; считает audiocpp_server движка озвучки.
        Component {
            id: "asr-crisperwhisper",
            title: "Распознавание речи: CrisperWhisper",
            note: "Whisper, дообученный на дословную запись, почти все языки мира; работает в движке озвучки — нужен и он; лицензия некоммерческая",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::plain("models/crisperwhisper2-turbo/LICENSE.md", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/LICENSE.md", 17_784, "42f43bf4dc72ef422f91f6ca143f4514e21a68b353940d99091c1addcd0aee79"),
                FileSpec::plain("models/crisperwhisper2-turbo/README.md", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/README.md", 10_974, "ddc82a301a1b59035eefcc1a8b2ec90d56257104df7365b00493c58135a6b029"),
                FileSpec::plain("models/crisperwhisper2-turbo/added_tokens.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/added_tokens.json", 35_319, "228ddf33a2c2b94033dc0fad8e7847b554b1c9f7d2054f5455ffd562b6a30dc7"),
                FileSpec::plain("models/crisperwhisper2-turbo/config.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/config.json", 1_211, "9d823d0eacddfc5ee920fac73897abfd29fbecac7d9e49b5d8e7f0bd63a56144"),
                FileSpec::plain("models/crisperwhisper2-turbo/generation_config.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/generation_config.json", 3_880, "e20fb70a44ce2b3cb3b2bbbea852e064638563b072719640a286d2a6865066e1"),
                FileSpec::plain("models/crisperwhisper2-turbo/merges.txt", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/merges.txt", 493_869, "2df2990a395e35e8dfbc7511e08c12d56018d8d04691e0133e5d63b21e154dc6"),
                FileSpec::plain("models/crisperwhisper2-turbo/model.safetensors", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/model.safetensors", 1_617_907_538, "0121a185b87ada7497f22dfbcd6c86b5f599b69dfcc8676e37f36447caf0b630"),
                FileSpec::plain("models/crisperwhisper2-turbo/normalizer.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/normalizer.json", 52_666, "bf1c507dc8724ca9cf9903640dacfb69dae2f00edee4f21ceba106a7392f26dd"),
                FileSpec::plain("models/crisperwhisper2-turbo/preprocessor_config.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/preprocessor_config.json", 357, "654cf18d3e163b948ceaf9766da56ce0b52de265d58673cf61c9376f126bd499"),
                FileSpec::plain("models/crisperwhisper2-turbo/special_tokens_map.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/special_tokens_map.json", 2_186, "baea4ea09372eb4fca86b4e4346139fd73cb807d5087e9de0948e971739c3e74"),
                FileSpec::plain("models/crisperwhisper2-turbo/tokenizer.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/tokenizer.json", 3_937_105, "00e6dce8d06acafefaea8d5d1e57e89c69b3237d08ecbe2bf6eb7124e248bcde"),
                FileSpec::plain("models/crisperwhisper2-turbo/tokenizer_config.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/tokenizer_config.json", 288_287, "3089ab61ce4745667a41e91620081cd3e7d61a9ae1f79fdf91b1bc8983916f4e"),
                FileSpec::plain("models/crisperwhisper2-turbo/vocab.json", "https://huggingface.co/nyralabs/CrisperWhisper2.0_turbo/resolve/de0369c8a68025b7f6e86387b6eb5a3b369787c8/vocab.json", 835_528, "6788c80b082e9b0d1393147d3a3e62ba19285ac0c82ace8e5ef00f37ead58971"),
            ],
        },
        Component {
            id: "asr-gpu",
            title: "Parakeet на видеокарте",
            note: "сборка transcribe.cpp под CUDA и её библиотеки; без неё Parakeet работает на процессоре",
            requirement: Requirement::Optional,
            variant_of: "",
            files: &[TCPP_CUDA, CUDART13, CUBLAS13],
        },
    ];

pub fn manifest() -> Vec<Component> {
    COMPONENTS.to_vec()
}

pub fn variant_selection(id: &str) -> Option<(&'static str, String)> {
    match id {
        "asr" => Some(("parakeet", "int8".into())),
        "asr-ultra" => Some(("parakeet", "ultra".into())),
        "asr-crisperwhisper" => Some((crate::runtime::ASR_CRISPERWHISPER, String::new())),
        _ => None,
    }
}

pub fn select_variant(runtime: &mut crate::runtime::Runtime, id: &str) -> bool {
    let Some((engine, model)) = variant_selection(id) else { return false };
    runtime.asr_engine = engine.to_string();
    if engine != crate::runtime::ASR_CRISPERWHISPER {
        runtime.asr_model = model;
    }
    true
}

pub fn variant_selected(runtime: &crate::runtime::Runtime, id: &str) -> bool {
    let crisper = runtime.asr_engine == crate::runtime::ASR_CRISPERWHISPER;
    match variant_selection(id) {
        Some((crate::runtime::ASR_CRISPERWHISPER, _)) => crisper,
        Some((_, model)) => !crisper && runtime.asr_model == model,
        None => false,
    }
}

/// Модель Parakeet выбранного варианта.
pub fn parakeet_model(runtime: &crate::runtime::Runtime) -> &'static dub_tcpp::Model {
    dub_tcpp::chosen("parakeet", Some(&runtime.asr_model)).expect("Parakeet v3 и Ultra есть в dub_tcpp::MODELS")
}

/// Прежние движки, которых никто больше не читает: Parakeet на ONNX Runtime (0.13 и раньше) с самой onnxruntime,
/// faster-whisper с его моделями и библиотеками CUDA 11, библиотека голоса Higgs (голос — в audiocpp_server).
const RETIRED_ASR: [&str; 6] = [
    "models/asr",
    "models/asr-ultra",
    "models/runtime/onnxruntime-win-x64-1.28.2",
    "tools/whisper",
    "models/whisper",
    "models/tts-engine/audiocpp_engine.dll",
];

/// Убрать то из [`RETIRED_ASR`], что осталось от прежней версии.
pub fn remove_retired_asr(root: &Path) {
    for rel in RETIRED_ASR {
        let path = root.join(rel);
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else if path.is_file() {
            std::fs::remove_file(&path)
        } else {
            continue;
        };
        match removed {
            Ok(()) => tracing::info!("убран прежний движок {}", path.display()),
            Err(error) => tracing::warn!("не убрать прежний движок {}: {error}", path.display()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatus {
    #[serde(flatten)]
    pub component: Component,
    pub present: bool,
    /// Сколько байт уже на диске: готовый файл или незавершённая докачка.
    pub have_bytes: u64,
    /// Вариант стадии, выбранный сейчас в настройках.
    pub selected: bool,
    /// Чем кончилась последняя закачка, если сорвалась: интерфейс снимает шкалу и говорит причину.
    pub error: Option<String>,
    /// Та же причина с кодом и полями — окно переводит её на язык игры.
    pub error_info: Option<serde_json::Value>,
    /// Программа (yue2, dub...), у которой компонент взят в этом запуске вместо закачки.
    pub taken_from: Option<String>,
}

/// Что идёт по сети прямо сейчас и чем сорвались прошлые закачки, по id компонента. Байты на
/// диске не скажут ни того, ни другого: сервер без докачки пишет файл без манифеста частей.
#[derive(Default)]
struct Downloads {
    live: std::collections::HashMap<&'static str, u64>,
    /// Текст сбоя и он же с кодом и полями, как его переводит окно.
    failed: std::collections::HashMap<&'static str, (String, serde_json::Value)>,
}

/// Закачка компонентов `ids` не началась: каждый из них показывает эту причину, как свой сорвавшийся файл.
pub fn record_failure(ids: &[String], error: &crate::errors::UserError) {
    let mut downloads = downloads();
    for component in manifest().into_iter().filter(|component| ids.iter().any(|id| id == component.id)) {
        downloads.failed.insert(component.id, (error.to_string(), error.to_json()));
    }
}

/// Новая закачка начинается без старых сбоев: иначе первый же опрос окна принял бы прошлую
/// ошибку за свежую.
pub fn forget_failures(ids: &[String]) {
    let mut downloads = downloads();
    downloads.failed.retain(|id, _| !ids.iter().any(|wanted| wanted == id));
}

fn downloads() -> std::sync::MutexGuard<'static, Downloads> {
    static DOWNLOADS: std::sync::OnceLock<Mutex<Downloads>> = std::sync::OnceLock::new();
    DOWNLOADS.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Есть ли этот файл на диске уже в готовом виде.
///
/// «Файл существует» — не то же самое, что «файл целый»: оборванная закачка оставляет
/// обрубок. Размер в манифесте точный до байта, поэтому прямой файл засчитывается только при
/// точном совпадении — так же старый файл прежней версии (другой движок голоса) считается
/// недостающим и докачивается. Архив после распаковки удаляется: смотрим на маркер и на метку
/// версии, которую распаковка кладёт рядом, — иначе движок прошлой версии считался бы новым.
/// Скачан ли компонент `id` целиком: так же, как его видит экран «Что скачать».
pub fn component_present(root: &Path, id: &str) -> bool {
    COMPONENTS.iter().find(|component| component.id == id).is_some_and(|component| component.files.iter().all(|file| file_present(root, file)))
}

fn file_present(root: &Path, file: &FileSpec) -> bool {
    if file.is_archive() {
        let marker = root.join(file.marker);
        let unpacked = if marker.extension().is_some() { marker.is_file() } else { voices_installed(&marker) };
        return unpacked && stamp_path(root, file).is_file();
    }
    let target = root.join(file.path);
    let Ok(meta) = std::fs::metadata(&target) else { return false };
    meta.is_file() && !part_path(&target).exists() && meta.len() == file.bytes
}

// ── Модели других программ timoncool на этом компьютере ─────────────────────

/// Сказать один раз на файл, почему копию соседа взять не вышло: статус спрашивают каждую секунду.
fn report_take_failure(target: &Path, app: &str, error: &std::io::Error) {
    static SAID: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    let mut said = SAID.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !said.iter().any(|known| known == target) {
        said.push(target.to_path_buf());
        tracing::warn!("{} не взят у {app}: {error}", target.display());
    }
}

/// Совпадает ли отпечаток файла с манифестом — без побочных действий `verify_sha256` (та стирает файл и манифест
/// докачки настоящей закачки).
fn sha256_matches(path: &Path, spec: &FileSpec, abort: &AtomicBool) -> bool {
    sha256_file(path, abort).ok().flatten().is_some_and(|got| got == spec.sha256)
}

/// Причина сорвавшейся закачки кодом для хаба: одна причина — одна строка, без путей и текста окна.
pub fn failure_code(error: &str) -> &'static str {
    if error.contains("контрольная сумма") {
        "hash_mismatch"
    } else if error.contains("os error 112") || error.contains("os error 28") {
        // ERROR_DISK_FULL на Windows, ENOSPC в Linux и macOS
        "disk_space"
    } else if error.contains("запись:") || error.contains("запись не продвинулась") {
        // раньше сетевых: ошибка записи посреди диапазона несёт и слово «диапазон»
        "disk"
    } else if error.contains("диапазон") || error.contains("не удалось узнать размер") {
        "network"
    } else if error.contains("архив") || error.contains("распаковка") {
        "archive"
    } else if ["запись", "создать", "открыть", "разметить", "финализация", "манифест", "метка", "прочитать"].iter().any(|word| error.contains(word)) {
        "disk"
    } else {
        "other"
    }
}

/// Сколько места ещё займёт файл: остаток закачки (файл `.part` размечается на полный размер сразу), а архив —
/// ещё и распакованным рядом с собой.
fn space_needed(root: &Path, file: &FileSpec) -> u64 {
    if file_present(root, file) {
        return 0;
    }
    let target = root.join(file.path);
    let started = std::fs::metadata(part_path(&target)).map(|meta| meta.len()).unwrap_or(0).min(file.bytes);
    let rest = file.bytes - started;
    if file.is_archive() { rest + 2 * file.bytes } else { rest }
}

/// Не хватает места под закачку компонентов `ids`: (нужно, свободно) в байтах. Проверка до первого байта — иначе
/// закачка гигабайтов обрывается посреди файла ошибкой записи.
pub fn missing_space(root: &Path, ids: &[String]) -> Option<(u64, u64)> {
    let need: u64 = manifest()
        .iter()
        .filter(|component| ids.iter().any(|id| id == component.id))
        .flat_map(|component| component.files.iter())
        .map(|file| space_needed(root, file))
        .sum();
    let free = free_bytes(root)?;
    (free < need).then_some((need, free))
}

/// Свободное для этого пользователя место на томе пути (с учётом квот). Путь может ещё не существовать — берётся
/// ближайший существующий каталог.
#[cfg(windows)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, free_to_caller: *mut u64, total: *mut u64, total_free: *mut u64) -> i32;
    }
    let dir = path.ancestors().find(|ancestor| ancestor.is_dir())?;
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut available = 0u64;
    // SAFETY: строка с нулём на конце живёт до конца вызова; необязательные выходы — нулевые указатели.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(available)
}

#[cfg(unix)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let dir = path.ancestors().find(|ancestor| ancestor.is_dir())?;
    let c_path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: путь с нулём на конце, структура принадлежит нам.
    let ok = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0;
    ok.then(|| stat.f_bavail as u64 * stat.f_frsize as u64)
}

/// Взятое у соседей в этом запуске: id компонента → программа, для пометки в окне.
fn lock_taken() -> std::sync::MutexGuard<'static, BTreeMap<String, String>> {
    static TAKEN: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
    TAKEN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn link_from_neighbors(root: &Path) {
    for component in manifest() {
        // что сейчас качается, не трогаем: его файл «отсутствует» лишь потому, что рядом лежит .part
        if downloads().live.contains_key(component.id) {
            continue;
        }
        let mut took = None;
        for file in component.files.iter().filter(|file| !file.is_archive() && !file_present(root, file)) {
            let target = root.join(file.path);
            if part_path(&target).exists() {
                continue;
            }
            let Some(name) = target.file_name() else { continue };
            if let Some((source, app)) = studio_hub_client::neighbors::find_file(name, file.bytes, target.parent().and_then(Path::file_name)) {
                let checked = |staged: &Path| sha256_matches(staged, file, &AtomicBool::new(false));
                match studio_hub_client::neighbors::take_file(&source, &target, false, checked) {
                    Ok(()) => took = Some(app),
                    Err(error) => report_take_failure(&target, &app, &error),
                }
            }
        }
        if let Some(app) = took {
            tracing::info!("{} взят у {app}, без закачки", component.id);
            lock_taken().insert(component.id.to_string(), app);
        }
    }
    // статус спрашивают каждую секунду, а пак сверяется по SHA-256 сотни файлов: не чаще раза в минуту, в такт тому,
    // как studio-hub-client заново обходит папки соседей
    static PACK_TRIED: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let due = {
        let mut tried = PACK_TRIED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let due = tried.is_none_or(|at| at.elapsed() >= std::time::Duration::from_secs(60));
        if due {
            *tried = Some(std::time::Instant::now());
        }
        due
    };
    if due && !downloads().live.contains_key("voice-pack") {
        if let Some(app) = take_voice_pack(root) {
            tracing::info!("voice-pack взят у {app}, без закачки");
            lock_taken().insert("voice-pack".to_string(), app);
        }
    }
}

/// Файлы архива пака голосов: имя, размер и SHA-256 каждого (тот же пак, что у Dub Studio и Speech Studio).
fn voice_pack_files() -> impl Iterator<Item = (&'static str, u64, &'static str)> {
    include_str!("voice_pack.tsv").lines().filter_map(|line| {
        let mut fields = line.split('\t');
        Some((fields.next()?, fields.next()?.parse().ok()?, fields.next()?))
    })
}

/// Пак голосов из папки `voices` соседней программы, где лежат все его файлы, каждый по SHA-256 (остальные голоса
/// той папки не трогаются; обход соседей папку голосов пропускает, поэтому она смотрится прямо); пак засчитывается
/// установленным меткой архива. Программа, у которой взят, или None.
fn take_voice_pack(root: &Path) -> Option<String> {
    let spec = COMPONENTS.iter().find(|component| component.id == "voice-pack")?.files.first()?;
    if file_present(root, spec) {
        return None;
    }
    let dir = root.join(spec.extract_dir);
    let abort = AtomicBool::new(false);
    let whole = |path: &Path, size: u64, sha256: &str| {
        path.metadata().is_ok_and(|meta| meta.len() == size) && sha256_file(path, &abort).ok().flatten().is_some_and(|got| got == sha256)
    };
    let missing: Vec<_> = voice_pack_files().filter(|(name, size, sha256)| !whole(&dir.join(name), *size, sha256)).collect();
    if missing.is_empty() {
        std::fs::write(stamp_path(root, spec), spec.url).ok();
        return None;
    }
    for neighbor in studio_hub_client::neighbors::current() {
        let Some(theirs) = ["voices", "models/voices"].iter().map(|folder| neighbor.data_dir.join(folder)).find(|folder| {
            missing.iter().all(|(name, size, _)| folder.join(name).metadata().is_ok_and(|meta| meta.len() == *size))
        }) else {
            continue;
        };
        std::fs::create_dir_all(&dir).ok()?;
        let taken = missing.iter().all(|(name, size, sha256)| {
            let target = dir.join(name);
            match studio_hub_client::neighbors::take_file(&theirs.join(name), &target, true, |staged| whole(staged, *size, sha256)) {
                Ok(()) => true,
                Err(error) => {
                    report_take_failure(&target, &neighbor.app, &error);
                    false
                }
            }
        });
        if taken {
            std::fs::write(stamp_path(root, spec), spec.url).ok()?;
            return Some(neighbor.app);
        }
    }
    None
}

/// Метка распакованного архива: имя несёт начало его SHA-256, поэтому новая версия архива
/// требует своей метки, а метка прошлой ничего не засчитывает.
fn stamp_path(root: &Path, file: &FileSpec) -> PathBuf {
    let name = Path::new(file.path).file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let version = file.sha256.get(..12).unwrap_or(file.sha256);
    root.join(file.extract_dir).join(format!(".{name}-{version}.installed"))
}

/// Потоковый SHA-256 файла блоками по 1 МиБ. None — остановлено флагом отмены: отмена не ждёт
/// конца хэширования многогигабайтного файла.
fn sha256_file(path: &Path, abort: &AtomicBool) -> std::io::Result<Option<String>> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut blocks = 0u32;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        blocks += 1;
        if blocks.is_multiple_of(64) && abort.load(Ordering::Relaxed) {
            return Ok(None);
        }
    }
    Ok(Some(digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()))
}

/// Сверить скачанное с отпечатком из манифеста. Не совпало — обрубок и манифест докачки
/// удаляются: возобновлять испорченный файл бессмысленно.
fn verify_sha256(part: &Path, spec: &FileSpec, abort: &AtomicBool) -> Result<(), String> {
    let got = sha256_file(part, abort)
        .map_err(|error| format!("прочитать {}: {error}", part.display()))?
        .ok_or_else(|| "отменено".to_string())?;
    if got != spec.sha256 {
        let _ = std::fs::remove_file(part);
        let _ = std::fs::remove_file(done_path(&part.with_extension("")));
        return Err(format!(
            "{}: контрольная сумма не совпала (ждали {}, получили {got}) — файл удалён, скачайте заново",
            spec.path, spec.sha256
        ));
    }
    Ok(())
}

/// Сколько байт этого файла уже лежит на диске: готовый файл или незавершённая докачка.
///
/// Незавершённый файл размечается на полный размер сразу, поэтому его размер ни о чём не
/// говорит: считаем по отметкам готовых диапазонов в манифесте докачки.
fn file_have_bytes(root: &Path, file: &FileSpec) -> u64 {
    if file_present(root, file) {
        return file.bytes;
    }
    let target = root.join(file.path);
    let chunks = completed_offsets(&done_path(&target)).len() as u64;
    (chunks * CHUNK).min(file.bytes)
}

pub fn status(root: &Path) -> Vec<ComponentStatus> {
    let runtime = crate::runtime::load(root);
    link_from_neighbors(root);
    let mut statuses: Vec<ComponentStatus> = manifest()
        .into_iter()
        .map(|component| {
            let present = component.files.iter().all(|file| file_present(root, file));
            let taken_from = if present { lock_taken().get(component.id).cloned() } else { None };
            let on_disk: u64 = component.files.iter().map(|file| file_have_bytes(root, file)).sum();
            let (live, error) = {
                let downloads = downloads();
                (downloads.live.get(component.id).copied(), downloads.failed.get(component.id).cloned())
            };
            let have_bytes = live.map_or(on_disk, |live| live.max(on_disk));
            let selected = variant_selected(&runtime, component.id);
            let (error, error_info) = error.map_or((None, None), |(text, info)| (Some(text), Some(info)));
            ComponentStatus { component, present, have_bytes, selected, error, error_info, taken_from }
        })
        .collect();
    // Стадия, уведённая в облако (или рассказчик — агенту), своих весов не требует: качать их
    // ради готовности незачем.
    let elsewhere = |id: &str| {
        let narrator = id.starts_with("narrator");
        let image = id.starts_with("image-");
        let voice = id.starts_with("voice-");
        let asr = !variant_of(id).is_empty();
        (narrator
            && (crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Narrator) || runtime.narrator_provider == "agent"))
            || (image && crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Image))
            || (voice && crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Tts))
            || (asr && crate::cloud::stage_enabled(&runtime, crate::cloud::Stage::Asr))
    };
    let accel = crate::accel::accel(&runtime);
    for status in statuses.iter_mut() {
        if status.component.requirement != Requirement::Optional
            && (elsewhere(status.component.id) || !fits_this_machine(status.component.id, accel))
        {
            status.component.requirement = Requirement::Optional;
        }
    }
    installed_variant_covers_group(&mut statuses);
    statuses
}

fn fits_this_machine(id: &str, accel: crate::accel::Accel) -> bool {
    use crate::accel::Accel;
    let (narrator, voice) = match accel {
        Accel::Cuda13 => ("narrator-engine", "voice-engine"),
        Accel::Cuda12 => ("narrator-engine-cuda12", "voice-engine-cuda12"),
        Accel::Vulkan => ("narrator-engine-vulkan", "voice-engine-vulkan"),
        Accel::Cpu => ("narrator-engine-cpu", "voice-engine-cpu"),
    };
    match id {
        "narrator-engine" | "narrator-engine-cuda12" | "narrator-engine-vulkan" | "narrator-engine-cpu" => id == narrator,
        "voice-engine" | "voice-engine-cuda12" | "voice-engine-vulkan" | "voice-engine-cpu" => id == voice,
        "image-engine" => accel.nvidia(),
        "image-engine-vulkan" => accel == Accel::Vulkan,
        _ if id.starts_with("image-") => accel != Accel::Cpu,
        "asr-gpu" => accel.nvidia(),
        _ => true,
    }
}

fn variant_of(id: &str) -> &'static str {
    COMPONENTS.iter().find(|component| component.id == id).map(|component| component.variant_of).unwrap_or("")
}

/// Варианты одной стадии взаимозаменяемы: скачанный вариант закрывает группу, и вариант по
/// умолчанию больше не числится нужным — иначе выбор на первом запуске упирается в докачку
/// варианта, который игрок не выбирал.
fn installed_variant_covers_group(statuses: &mut [ComponentStatus]) {
    let covered: std::collections::BTreeSet<&'static str> = statuses
        .iter()
        .filter(|status| status.present && !status.component.variant_of.is_empty())
        .map(|status| status.component.variant_of)
        .collect();
    for status in statuses.iter_mut() {
        if !status.present && covered.contains(status.component.variant_of) {
            status.component.requirement = Requirement::Optional;
        }
    }
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

/// Размер файла и поддержка докачки по диапазонам: однобайтовый пробник, с повторами, как у диапазона.
fn probe_size(agent: &ureq::Agent, url: &str) -> Result<(u64, bool), String> {
    let mut last = String::new();
    let mut unreachable = 0;
    let mut attempts = 0;
    for attempt in 0..RANGE_RETRIES {
        attempts = attempt + 1;
        match agent.get(url).header("Range", "bytes=0-0").call() {
            Ok(response) if response.status().as_u16() == 206 => {
                let total = response
                    .headers()
                    .get("content-range")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.rsplit('/').next())
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(0);
                if total > 0 {
                    return Ok((total, true));
                }
                last = "в ответе нет размера (content-range)".into();
            }
            Ok(response) => {
                let total = response
                    .headers()
                    .get("content-length")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(0);
                if total > 0 {
                    return Ok((total, false));
                }
                last = format!("ответ {} без размера", response.status());
            }
            Err(error) => {
                last = error.to_string();
                if cannot_connect(&error) {
                    unreachable += 1;
                    if unreachable == CONNECT_RETRIES {
                        break;
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(400 * (attempt as u64 + 1)));
    }
    Err(start_failure(url, attempts, unreachable == CONNECT_RETRIES, &last))
}

/// Запрос не дошёл до сервера: соединение не открылось — не ответ и не оборванная передача.
fn cannot_connect(error: &ureq::Error) -> bool {
    use std::io::ErrorKind;
    match error {
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed | ureq::Error::ConnectProxyFailed(_) => true,
        ureq::Error::Timeout(timeout) => matches!(timeout, ureq::Timeout::Connect | ureq::Timeout::Resolve),
        ureq::Error::Io(error) => matches!(
            error.kind(),
            ErrorKind::ConnectionRefused | ErrorKind::AddrNotAvailable | ErrorKind::NetworkUnreachable | ErrorKind::HostUnreachable
        ),
        _ => false,
    }
}

/// Почему закачка не пошла. Хост назван отдельно от адреса: статистика адреса вырезает, а какой сервер не
/// достаётся из сети игрока — весь вопрос.
fn start_failure(url: &str, attempts: u32, unreachable: bool, last: &str) -> String {
    let host = url.split("://").nth(1).and_then(|rest| rest.split(['/', ':']).next()).unwrap_or_default();
    if unreachable {
        format!("нет соединения с {host} ({attempts} попытки) для {url}: {last}")
    } else {
        format!("{url} с {host} не дался за {attempts} попыток: {last}")
    }
}

/// Какие смещения уже скачаны — читается из манифеста докачки.
fn completed_offsets(path: &Path) -> Vec<u64> {
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    bytes.as_chunks::<8>().0.iter().map(|chunk| u64::from_le_bytes(*chunk)).collect()
}

/// Скачать один диапазон с проверкой полноты и повторами.
///
/// Входов много, и каждый нужен: куда писать, что качать, сколько уже скачано, не отменили
/// ли всё и куда отметить готовое. Заворачивать их в структуру ради счётчика аргументов —
/// прятать смысл за именем.
#[allow(clippy::too_many_arguments)]
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
    let mut unreachable = 0;
    let mut attempts = 0;

    for attempt in 0..RANGE_RETRIES {
        if abort.load(Ordering::Relaxed) {
            return Err("отменено".into());
        }
        attempts = attempt + 1;
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
            Err((error, _)) if error == "отменено" => return Err(error),
            Err((error, no_connection)) => {
                last = error;
                if no_connection {
                    unreachable += 1;
                }
            }
        }
        // Откатываем вклад неудачной попытки, иначе повтор задвоил бы прогресс.
        downloaded.fetch_sub(got.min(downloaded.load(Ordering::Relaxed)), Ordering::Relaxed);
        if unreachable == CONNECT_RETRIES {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(400 * (attempt as u64 + 1)));
    }
    Err(format!("диапазон {start}-{end}: {}", start_failure(url, attempts, unreachable == CONNECT_RETRIES, &last)))
}

/// Одна попытка диапазона. Ошибка несёт признак, что соединение с хостом не открылось вовсе.
#[allow(clippy::too_many_arguments)]
fn download_range_once(
    agent: &ureq::Agent,
    url: &str,
    file: &Arc<File>,
    start: u64,
    end: u64,
    downloaded: &Arc<AtomicU64>,
    abort: &Arc<AtomicBool>,
    got: &mut u64,
) -> Result<(), (String, bool)> {
    let response = agent
        .get(url)
        .header("Range", &format!("bytes={start}-{end}"))
        .call()
        .map_err(|error| (format!("диапазон {start}-{end}: {error}"), cannot_connect(&error)))?;
    if response.status().as_u16() != 206 {
        return Err((format!("диапазон {start}-{end}: ответ {}", response.status()), false));
    }

    let mut reader = response.into_body().into_reader();
    let mut buffer = [0u8; 262_144];
    let mut offset = start;
    loop {
        if abort.load(Ordering::Relaxed) {
            return Err(("отменено".into(), false));
        }
        let read = reader.read(&mut buffer).map_err(|error| (format!("чтение: {error}"), false))?;
        if read == 0 {
            break;
        }
        let mut written = 0;
        while written < read {
            let step = write_at(file, &buffer[written..read], offset + written as u64)
                .map_err(|error| (format!("запись: {error}"), false))?;
            if step == 0 {
                return Err(("запись не продвинулась".into(), false));
            }
            written += step;
        }
        offset += read as u64;
        *got += read as u64;
        downloaded.fetch_add(read as u64, Ordering::Relaxed);
    }
    Ok(())
}

/// Скачать компонент целиком: все его файлы по очереди, с общим прогрессом.
/// `progress` зовут с числом уже полученных байт.
pub fn download_component(
    root: &Path,
    component: &Component,
    progress: &dyn Fn(u64, u64),
    abort: Arc<AtomicBool>,
) -> Result<(), String> {
    downloads().failed.remove(component.id);
    let id = component.id;
    let tracked = |have: u64, total: u64| {
        downloads().live.insert(id, have);
        progress(have, total);
    };
    let result = download_files(root, component, &tracked, abort);
    let mut downloads = downloads();
    downloads.live.remove(id);
    if let Err(error) = &result {
        let coded = crate::errors::UserError::Download { kind: failure_code(error), detail: error.clone() }.to_json();
        downloads.failed.insert(id, (error.clone(), coded));
    }
    result
}

fn download_files(
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

    // у соседней программы файл уже есть: берём его, копией тоже, если она на другом диске, — и сверяем как закачку
    if !spec.is_archive() {
        let found = target.file_name().and_then(|name| studio_hub_client::neighbors::find_file(name, spec.bytes, None));
        if let Some((source, app)) = found {
            let taken = studio_hub_client::neighbors::take_file(&source, &target, true, |staged| sha256_matches(staged, spec, &abort));
            if let Err(error) = &taken {
                if abort.load(Ordering::SeqCst) {
                    studio_hub_client::neighbors::reconsider(&source);
                    return Err("отменено".into());
                }
                report_take_failure(&target, &app, error);
            }
            if taken.is_ok() {
                tracing::info!("{title} взят у {app}, без закачки");
                if let Some(component) = manifest().into_iter().find(|component| component.files.iter().any(|file| file.path == spec.path)) {
                    lock_taken().insert(component.id.to_string(), app);
                }
                progress(spec.bytes, spec.bytes);
                return Ok(());
            }
        }
    }

    let agent = ureq::Agent::new_with_defaults();
    let (total, ranged) = probe_size(&agent, spec.url).map_err(|error| format!("не удалось узнать размер {title}: {error}"))?;

    let part = part_path(&target);
    let done_file = done_path(&target);
    let file = Arc::new(
        OpenOptions::new()
            .create(true)
            // Обрезать НЕЛЬЗЯ: файл открывается и на продолжение прерванной загрузки,
            // а обрезание стёрло бы уже скачанные гигабайты.
            .truncate(false)
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
        verify_sha256(&part, spec, &abort)?;
        std::fs::rename(&part, &target).map_err(|error| error.to_string())?;
        // Тот же финал, что у докачки диапазонами: без распаковки архив так и лежал бы
        // архивом, маркер не появлялся бы, и компонент качался бы заново при каждом старте.
        unpack_if_archive(root, spec, &target)?;
        progress(total, total);
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
    drop(done);
    // Манифест докачки живёт до конца сверки: прерванная сверка не должна стоить всей закачки.
    verify_sha256(&part, spec, &abort)?;
    std::fs::rename(&part, &target).map_err(|error| format!("финализация: {error}"))?;
    let _ = std::fs::remove_file(&done_file);
    unpack_if_archive(root, spec, &target)?;
    progress(total, total);
    Ok(())
}

/// Архив разворачивается сразу после закачки; сам он после этого только занимал бы место.
/// Метка версии кладётся последней: пока её нет, архив не считается установленным.
fn unpack_if_archive(root: &Path, spec: &FileSpec, target: &Path) -> Result<(), String> {
    if !spec.is_archive() {
        return Ok(());
    }
    let dir = root.join(spec.extract_dir);
    unpack(target, &dir, spec.extract)?;
    let _ = std::fs::remove_file(target);
    std::fs::write(stamp_path(root, spec), spec.url).map_err(|error| format!("метка установки: {error}"))
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

/// Распаковать архив. Плоско — берём только имя файла, вложенные каталоги схлопываем; поддеревом —
/// путь записи сохраняется, но только безопасный (`enclosed_name`): выход за каталог назначения
/// отвергается; только библиотеки — плоско и лишь `.dll`.
fn unpack(archive: &Path, dir: &Path, mode: Extract) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| format!("открыть архив: {error}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| format!("это не архив: {error}"))?;
    std::fs::create_dir_all(dir).map_err(|error| format!("создать каталог: {error}"))?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| format!("запись архива: {error}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|error| format!("имя записи архива: {error}"))?
            .replace('\\', "/");
        let leaf = name.rsplit('/').next().unwrap_or_default().to_string();
        if leaf.is_empty() || leaf == "." || leaf == ".." {
            continue;
        }
        let out = match mode {
            Extract::ZipTree => match entry.enclosed_name() {
                Some(relative) => dir.join(relative),
                None => return Err(format!("запись архива ведёт за каталог назначения: {name}")),
            },
            Extract::ZipDlls if !leaf.to_ascii_lowercase().ends_with(".dll") => continue,
            // Имя с диском («C:x.dll») заменило бы каталог назначения при join.
            _ if !matches!(Path::new(&leaf).components().collect::<Vec<_>>()[..], [std::path::Component::Normal(_)]) => {
                return Err(format!("запись архива ведёт за каталог назначения: {name}"));
            }
            _ => dir.join(&leaf),
        };
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("создать каталог: {error}"))?;
        }
        let mut target = File::create(&out).map_err(|error| format!("создать {leaf}: {error}"))?;
        std::io::copy(&mut entry, &mut target)
            .map_err(|error| format!("распаковка {leaf}: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_voice_pack_lists_every_file_of_its_archive() {
        let files: Vec<_> = super::voice_pack_files().collect();
        assert_eq!(files.len(), 100);
        assert!(files.iter().all(|(name, size, sha)| !name.is_empty() && *size > 0 && sha.len() == 64));
    }

    /// Parakeet и сборки transcribe.cpp ложатся туда, где их ищут `dub_tcpp` и голосовой ввод, по путям Dub Studio;
    /// распознавание на процессоре не тянет файлов CUDA.
    #[test]
    fn parakeet_files_land_where_the_binding_looks() {
        use super::{parakeet_model, Extract, COMPONENTS};
        let models = std::path::Path::new("models");
        for (id, variant) in [("asr", "int8"), ("asr-ultra", "ultra")] {
            let runtime = crate::runtime::Runtime { asr_engine: "parakeet".into(), asr_model: variant.into(), ..Default::default() };
            let model = parakeet_model(&runtime);
            let files = COMPONENTS.iter().find(|c| c.id == id).unwrap().files;
            assert!(files.iter().any(|f| std::path::Path::new(f.path) == dub_tcpp::model_path(models, model) && f.sha256 == model.download.sha256), "{id}");
            let engine = |build: dub_tcpp::Build| dub_tcpp::engine_dir(models, build).join(dub_tcpp::LIBRARY);
            let has = |files: &[super::FileSpec], build: dub_tcpp::Build| files.iter().any(|f| std::path::Path::new(f.path) == engine(build) && f.extract == Extract::None && f.sha256 == build.download().sha256);
            assert!(has(files, dub_tcpp::Build::Cpu) && !has(files, dub_tcpp::Build::Cuda), "{id}: the processor build comes with the model, the card's apart");
            assert!(files.iter().all(|f| !f.url.contains("onnx") && !f.url.contains("nvidia")), "{id}");
        }
        let gpu = COMPONENTS.iter().find(|c| c.id == "asr-gpu").unwrap();
        assert!(gpu.files.iter().any(|f| std::path::Path::new(f.path) == dub_tcpp::engine_dir(models, dub_tcpp::Build::Cuda).join(dub_tcpp::LIBRARY)));
        assert!(gpu.files.iter().any(|f| f.marker.ends_with("cublas64_13.dll")) && gpu.files.iter().any(|f| f.marker.ends_with("cudart64_13.dll")));
    }

    #[test]
    fn a_failed_download_reaches_the_hub_as_a_code() {
        use super::failure_code;
        assert_eq!(failure_code("models/x.gguf: контрольная сумма не совпала (ждали a, получили b)"), "hash_mismatch");
        assert_eq!(failure_code("диапазон 0-100 не дался за 5 попыток: timeout"), "network");
        assert_eq!(failure_code("создать каталог: Отказано в доступе"), "disk");
        assert_eq!(failure_code("финализация: There is not enough space on the disk. (os error 112)"), "disk_space");
        assert_eq!(failure_code("диапазон 0-9: x не дался за 8 попыток: запись: Отказано в доступе"), "disk");
        assert_eq!(failure_code("не удалось узнать размер X: нет соединения с huggingface.co (3 попытки) для u: x"), "network");
        assert_eq!(failure_code("запись архива ведёт за каталог назначения: a"), "archive");
    }

    /// Хост, где никто не слушает, бросается после трёх попыток, а не восьми, и ошибка называет его рядом с адресом.
    #[test]
    fn a_host_that_cannot_be_reached_is_named_and_not_retried_eight_times() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = closed.local_addr().unwrap().port();
        drop(closed);
        let url = format!("http://127.0.0.1:{port}/model.gguf");
        let error = probe_size(&ureq::Agent::new_with_defaults(), &url).unwrap_err();
        assert!(error.starts_with(&format!("нет соединения с 127.0.0.1 (3 попытки) для {url}: ")), "{error}");
        assert_eq!(start_failure("https://github.com/a/b.zip", 8, false, "x"), "https://github.com/a/b.zip с github.com не дался за 8 попыток: x");
        assert_eq!(failure_code("это не архив: x.whl"), "archive");
        assert_eq!(failure_code("что-то ещё"), "other");
    }

    #[test]
    fn a_file_counts_only_at_its_exact_size() {
        use super::{file_present, FileSpec};
        let dir = tempfile::tempdir().unwrap();
        let spec = FileSpec::plain("модель.gguf", "https://example/модель.gguf", 10, "00");

        std::fs::write(dir.path().join("модель.gguf"), b"12345").unwrap();
        assert!(!file_present(dir.path(), &spec), "обрубок не считается установленным");
        std::fs::write(dir.path().join("модель.gguf"), b"12345678901").unwrap();
        assert!(!file_present(dir.path(), &spec), "файл прежней версии другого размера докачивается");
        std::fs::write(dir.path().join("модель.gguf"), b"1234567890").unwrap();
        assert!(file_present(dir.path(), &spec));

        std::fs::write(dir.path().join("модель.gguf.part"), b"").unwrap();
        assert!(!file_present(dir.path(), &spec), "рядом незавершённая закачка — файл не готов");
    }

    #[test]
    fn every_file_is_pinned_and_fingerprinted() {
        for component in super::manifest() {
            for spec in component.files {
                assert!(!spec.url.contains("/resolve/main/"), "{}: {} не закреплён", component.id, spec.url);
                if spec.url.contains("huggingface.co") {
                    let revision = spec.url.split("/resolve/").nth(1).and_then(|tail| tail.split('/').next()).unwrap_or("");
                    assert!(
                        revision.len() == 40 && revision.chars().all(|c| c.is_ascii_hexdigit()),
                        "{}: ревизия {revision} не коммит",
                        component.id
                    );
                }
                assert!(
                    spec.sha256.len() == 64 && spec.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                    "{}: у {} нет SHA-256",
                    component.id,
                    spec.path
                );
            }
        }
    }

    #[test]
    fn a_wrong_checksum_deletes_the_file() {
        use super::{verify_sha256, FileSpec};
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("модель.gguf.part");
        std::fs::write(&part, b"abc").unwrap();
        let abort = std::sync::atomic::AtomicBool::new(false);
        // SHA-256 строки «abc».
        let good = FileSpec::plain(
            "модель.gguf",
            "https://example/модель.gguf",
            3,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
        assert!(verify_sha256(&part, &good, &abort).is_ok());
        let bad = FileSpec::plain("модель.gguf", "https://example/модель.gguf", 3, "00");
        let error = verify_sha256(&part, &bad, &abort).unwrap_err();
        assert!(error.contains("контрольная сумма"), "{error}");
        assert!(!part.exists(), "испорченный файл удалён");
    }

    #[test]
    fn an_unpacked_archive_counts_only_with_the_stamp_of_its_version() {
        use super::{file_present, stamp_path, Extract, FileSpec};
        let dir = tempfile::tempdir().unwrap();
        let spec = FileSpec::archive(
            "downloads/llama.zip",
            "https://example/llama.zip",
            10,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Extract::ZipFlat,
            "tools/llama",
            "tools/llama/llama-server.exe",
        );
        std::fs::create_dir_all(dir.path().join("tools/llama")).unwrap();
        std::fs::write(dir.path().join("tools/llama/llama-server.exe"), b"old").unwrap();
        assert!(!file_present(dir.path(), &spec), "движок прошлой версии не засчитывается");
        std::fs::write(stamp_path(dir.path(), &spec), b"").unwrap();
        assert!(file_present(dir.path(), &spec));
    }

    #[test]
    fn a_tree_archive_cannot_write_outside_its_folder() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("evil.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        writer.start_file("../outside.dll", zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(b"x").unwrap();
        writer.finish().unwrap();
        let error = super::unpack(&archive, &dir.path().join("out"), super::Extract::ZipTree).unwrap_err();
        assert!(error.contains("за каталог"), "{error}");
        assert!(!dir.path().join("outside.dll").exists());
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
                if spec.is_archive() {
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
        // выглядел бы обнулившимся. Считаются готовые диапазоны из манифеста докачки.
        let target = dir.path().join("models/text/narrator.gguf");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(part_path(&target), vec![0u8; 1024]).unwrap();
        let mut done = Vec::new();
        done.extend_from_slice(&0u64.to_le_bytes());
        done.extend_from_slice(&CHUNK.to_le_bytes());
        std::fs::write(done_path(&target), done).unwrap();
        let statuses = status(dir.path());
        let narrator = statuses.iter().find(|status| status.component.id == "narrator").unwrap();
        assert!(!narrator.present);
        assert_eq!(narrator.have_bytes, 2 * CHUNK);
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
