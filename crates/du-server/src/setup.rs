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

/// Что делать с файлом после закачки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Extract {
    /// Ничего: файл кладётся как есть.
    None,
    /// Zip распаковывается ПЛОСКО в каталог назначения: паки приходят и с папкой внутри,
    /// и без неё, а раскладка должна получаться одинаковая.
    ZipFlat,
    /// Zip распаковывается с сохранением поддерева: onnxruntime ищется ровно по
    /// `onnxruntime-*/lib/…`.
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
                FileSpec::archive("downloads/llama.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11557/llama-b11557-bin-win-cuda-13.4-x64.zip", 153_446_322, "9a0d41f868d34b1c39a9dc4f0b2250d1ef32fb2aed978c54f4380b06a21b8d9f", Extract::ZipFlat, "tools/llama", "tools/llama/llama-server.exe"),
                FileSpec::archive("downloads/llama-cuda.zip", "https://github.com/ggml-org/llama.cpp/releases/download/b11557/cudart-llama-bin-win-cuda-13.4-x64.zip", 423_535_356, "738f8c251ac22b70c3ae6f83a10cf222725df0395246a2cf58f32bdb85fbe668", Extract::ZipFlat, "tools/llama", "tools/llama/cudart64_13.dll"),
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
            note: "Higgs: читает текст голосом из пака",
            requirement: Requirement::Recommended,
            variant_of: "",
            files: &[
                FileSpec::plain("models/tts-engine/audiocpp_engine.dll", "https://github.com/timoncool/Higgs-Ultimate/releases/download/engine-0.2.3-turing/audiocpp_engine.dll", 129_390_080, "c3608613bd54bdd41c85cc90412d4add67b928b07b32be24f26d91cb9626f46d"),
                // the engine imports cublas64_13.dll, which loads cudart64_13 and cublasLt64_13: without them beside it Windows refuses the DLL
                FileSpec::archive("downloads/tts-cudart.whl", "https://files.pythonhosted.org/packages/86/00/d5436004268f049214193659ebc36550b5ef3925c3d13b4cc980e13be6f5/nvidia_cuda_runtime-13.4.92-py3-none-win_amd64.whl", 2_778_543, "08dca5e4aba480c2fd5b55075c0fa71b84ef9dcf0521f2d58baa14a803a7311c", Extract::ZipDlls, "models/tts-engine", "models/tts-engine/cudart64_13.dll"),
                FileSpec::archive("downloads/tts-cublas.whl", "https://files.pythonhosted.org/packages/a3/df/f1246959833e2c437db8be3e5b477f66b87f8817821ed40de6c7561c9a36/nvidia_cublas-13.8.0.4-py3-none-win_amd64.whl", 423_266_897, "8c5494423bb8a46822cb6b0cb95d7fa4be2d7b96a31155dff083839ec8297910", Extract::ZipDlls, "models/tts-engine", "models/tts-engine/cublas64_13.dll"),
            ],
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
                FileSpec::archive("downloads/onnxruntime.zip", "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-win-x64-1.28.2.zip", 78_620_837, "c4eedd29489d5feca21866d054638416f3655bf6b18851b3b6b85c8313e95c35", Extract::ZipTree, "models/runtime", "models/runtime/onnxruntime-win-x64-1.28.2/lib/onnxruntime.dll"),
                FileSpec::plain("models/asr/encoder-model.int8.onnx", "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/encoder-model.int8.onnx", 652_183_999, "6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09"),
                FileSpec::plain("models/asr/decoder_joint-model.int8.onnx", "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/decoder_joint-model.int8.onnx", 18_202_004, "eea7483ee3d1a30375daedc8ed83e3960c91b098812127a0d99d1c8977667a70"),
                FileSpec::plain("models/asr/nemo128.onnx", "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/nemo128.onnx", 139_764, "a9fde1486ebfcc08f328d75ad4610c67835fea58c73ba57e3209a6f6cf019e9f"),
                FileSpec::plain("models/asr/vocab.txt", "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/vocab.txt", 93_939, "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d"),
                FileSpec::plain("models/asr/config.json", "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/config.json", 97, "666903c76b9798caf2c210afd4f6cd60b08a8dbf9800ec8d7a3bc0d2148ac466"),
            ],
        },
        Component {
            id: "asr-ultra",
            title: "Распознавание речи: Parakeet Ultra",
            note: "та же модель, дообученная Moondream: точнее на тех же 25 языках",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::archive("downloads/onnxruntime.zip", "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-win-x64-1.28.2.zip", 78_620_837, "c4eedd29489d5feca21866d054638416f3655bf6b18851b3b6b85c8313e95c35", Extract::ZipTree, "models/runtime", "models/runtime/onnxruntime-win-x64-1.28.2/lib/onnxruntime.dll"),
                FileSpec::plain("models/asr-ultra/encoder-model.int8.onnx", "https://huggingface.co/Masterx/parakeet-tdt-0.6b-ultra-onnx/resolve/99b09f030a5a6efeaa13cf2cf54592100ce2c3f1/encoder-model.int8.onnx", 652_183_214, "46e78f85f1ae43b43bd359889c966da5c7ca401df60a42eeffc2e7699b357ba6"),
                FileSpec::plain("models/asr-ultra/decoder_joint-model.int8.onnx", "https://huggingface.co/Masterx/parakeet-tdt-0.6b-ultra-onnx/resolve/99b09f030a5a6efeaa13cf2cf54592100ce2c3f1/decoder_joint-model.int8.onnx", 18_202_004, "2276a335d4c8dc48686e931475a634595954956427d485830e1214c0d1a18d07"),
                FileSpec::plain("models/asr-ultra/nemo128.onnx", "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/nemo128.onnx", 139_764, "a9fde1486ebfcc08f328d75ad4610c67835fea58c73ba57e3209a6f6cf019e9f"),
                FileSpec::plain("models/asr-ultra/vocab.txt", "https://huggingface.co/Masterx/parakeet-tdt-0.6b-ultra-onnx/resolve/99b09f030a5a6efeaa13cf2cf54592100ce2c3f1/vocab.txt", 93_939, "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d"),
                FileSpec::plain("models/asr-ultra/config.json", "https://huggingface.co/Masterx/parakeet-tdt-0.6b-ultra-onnx/resolve/99b09f030a5a6efeaa13cf2cf54592100ce2c3f1/config.json", 97, "666903c76b9798caf2c210afd4f6cd60b08a8dbf9800ec8d7a3bc0d2148ac466"),
            ],
        },
        Component {
            id: "whisper-tiny",
            title: "Распознавание речи: Whisper tiny",
            note: "самая лёгкая, для слабого компьютера; понимает почти все языки мира",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::archive("downloads/whisper.zip", "https://github.com/Purfview/whisper-standalone-win/releases/download/faster-whisper/Whisper-Faster_r192.3_windows.zip", 87_654_143, "8150ad257fd8e46d817bb7e667260c2ce4c493d9e58973862e6409c592b44ba5", Extract::ZipFlat, "tools/whisper", "tools/whisper/whisper-faster.exe"),
                FileSpec::plain("models/whisper/faster-whisper-tiny/model.bin", "https://huggingface.co/Systran/faster-whisper-tiny/resolve/d90ca5fe260221311c53c58e660288d3deb8d356/model.bin", 75_538_270, "dcb76c6586fc06cbdac6dd21f14cfd129cc4cdd9dce19bf4ffa62e59cbe6e6d1"),
                FileSpec::plain("models/whisper/faster-whisper-tiny/config.json", "https://huggingface.co/Systran/faster-whisper-tiny/resolve/d90ca5fe260221311c53c58e660288d3deb8d356/config.json", 2_249, "a73a28cdfe1c43ccc7202fa333d1f89c202477271407ae9a7f19afa52039cac8"),
                FileSpec::plain("models/whisper/faster-whisper-tiny/tokenizer.json", "https://huggingface.co/Systran/faster-whisper-tiny/resolve/d90ca5fe260221311c53c58e660288d3deb8d356/tokenizer.json", 2_203_239, "fb7b63191e9bb045082c79fd742a3106a12c99513ab30df4a0d47fa6cb6fd0ab"),
                FileSpec::plain("models/whisper/faster-whisper-tiny/vocabulary.txt", "https://huggingface.co/Systran/faster-whisper-tiny/resolve/d90ca5fe260221311c53c58e660288d3deb8d356/vocabulary.txt", 459_861, "34ce3fe1c5041027b3f8d42912270993f986dbc4bb34cf27f951e34a1e453913"),
            ],
        },
        Component {
            id: "whisper-base",
            title: "Распознавание речи: Whisper base",
            note: "лёгкая; понимает почти все языки мира",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::archive("downloads/whisper.zip", "https://github.com/Purfview/whisper-standalone-win/releases/download/faster-whisper/Whisper-Faster_r192.3_windows.zip", 87_654_143, "8150ad257fd8e46d817bb7e667260c2ce4c493d9e58973862e6409c592b44ba5", Extract::ZipFlat, "tools/whisper", "tools/whisper/whisper-faster.exe"),
                FileSpec::plain("models/whisper/faster-whisper-base/model.bin", "https://huggingface.co/Systran/faster-whisper-base/resolve/ebe41f70d5b6dfa9166e2c581c45c9c0cfc57b66/model.bin", 145_217_532, "d01c3014881c9c6f3133c182f3d2887eb6ca1c789a7538c5c007196857a0a6a9"),
                FileSpec::plain("models/whisper/faster-whisper-base/config.json", "https://huggingface.co/Systran/faster-whisper-base/resolve/ebe41f70d5b6dfa9166e2c581c45c9c0cfc57b66/config.json", 2_309, "56a6d8110d311f19c8f0471e562832c7527f146b567275bfca59fcf7c184da9a"),
                FileSpec::plain("models/whisper/faster-whisper-base/tokenizer.json", "https://huggingface.co/Systran/faster-whisper-base/resolve/ebe41f70d5b6dfa9166e2c581c45c9c0cfc57b66/tokenizer.json", 2_203_239, "fb7b63191e9bb045082c79fd742a3106a12c99513ab30df4a0d47fa6cb6fd0ab"),
                FileSpec::plain("models/whisper/faster-whisper-base/vocabulary.txt", "https://huggingface.co/Systran/faster-whisper-base/resolve/ebe41f70d5b6dfa9166e2c581c45c9c0cfc57b66/vocabulary.txt", 459_861, "34ce3fe1c5041027b3f8d42912270993f986dbc4bb34cf27f951e34a1e453913"),
            ],
        },
        Component {
            id: "whisper-small",
            title: "Распознавание речи: Whisper small",
            note: "средняя по весу и точности; понимает почти все языки мира",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::archive("downloads/whisper.zip", "https://github.com/Purfview/whisper-standalone-win/releases/download/faster-whisper/Whisper-Faster_r192.3_windows.zip", 87_654_143, "8150ad257fd8e46d817bb7e667260c2ce4c493d9e58973862e6409c592b44ba5", Extract::ZipFlat, "tools/whisper", "tools/whisper/whisper-faster.exe"),
                FileSpec::plain("models/whisper/faster-whisper-small/model.bin", "https://huggingface.co/Systran/faster-whisper-small/resolve/536b0662742c02347bc0e980a01041f333bce120/model.bin", 483_546_902, "3e305921506d8872816023e4c273e75d2419fb89b24da97b4fe7bce14170d671"),
                FileSpec::plain("models/whisper/faster-whisper-small/config.json", "https://huggingface.co/Systran/faster-whisper-small/resolve/536b0662742c02347bc0e980a01041f333bce120/config.json", 2_370, "b55496ac7940a7ae47d2c01eab40edfd8701feec1229d9cce3b40014383fb828"),
                FileSpec::plain("models/whisper/faster-whisper-small/tokenizer.json", "https://huggingface.co/Systran/faster-whisper-small/resolve/536b0662742c02347bc0e980a01041f333bce120/tokenizer.json", 2_203_239, "fb7b63191e9bb045082c79fd742a3106a12c99513ab30df4a0d47fa6cb6fd0ab"),
                FileSpec::plain("models/whisper/faster-whisper-small/vocabulary.txt", "https://huggingface.co/Systran/faster-whisper-small/resolve/536b0662742c02347bc0e980a01041f333bce120/vocabulary.txt", 459_861, "34ce3fe1c5041027b3f8d42912270993f986dbc4bb34cf27f951e34a1e453913"),
            ],
        },
        Component {
            id: "whisper-medium",
            title: "Распознавание речи: Whisper medium",
            note: "точнее, тяжелее; понимает почти все языки мира",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::archive("downloads/whisper.zip", "https://github.com/Purfview/whisper-standalone-win/releases/download/faster-whisper/Whisper-Faster_r192.3_windows.zip", 87_654_143, "8150ad257fd8e46d817bb7e667260c2ce4c493d9e58973862e6409c592b44ba5", Extract::ZipFlat, "tools/whisper", "tools/whisper/whisper-faster.exe"),
                FileSpec::plain("models/whisper/faster-whisper-medium/model.bin", "https://huggingface.co/Systran/faster-whisper-medium/resolve/08e178d48790749d25932bbc082711ddcfdfbc4f/model.bin", 1_527_906_378, "9b45e1009dcc4ab601eff815b61d80e60ce3fd8c74c1a14f4a282258286b51ae"),
                FileSpec::plain("models/whisper/faster-whisper-medium/config.json", "https://huggingface.co/Systran/faster-whisper-medium/resolve/08e178d48790749d25932bbc082711ddcfdfbc4f/config.json", 2_257, "3622a2ddc41ec0e0fd4e68c13c6830f03b90c38d89aaad184de02c8c642cf807"),
                FileSpec::plain("models/whisper/faster-whisper-medium/tokenizer.json", "https://huggingface.co/Systran/faster-whisper-medium/resolve/08e178d48790749d25932bbc082711ddcfdfbc4f/tokenizer.json", 2_203_239, "fb7b63191e9bb045082c79fd742a3106a12c99513ab30df4a0d47fa6cb6fd0ab"),
                FileSpec::plain("models/whisper/faster-whisper-medium/vocabulary.txt", "https://huggingface.co/Systran/faster-whisper-medium/resolve/08e178d48790749d25932bbc082711ddcfdfbc4f/vocabulary.txt", 459_861, "34ce3fe1c5041027b3f8d42912270993f986dbc4bb34cf27f951e34a1e453913"),
            ],
        },
        Component {
            id: "whisper-large-v3",
            title: "Распознавание речи: Whisper large-v3",
            note: "самая точная из Whisper; понимает почти все языки мира",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::archive("downloads/whisper.zip", "https://github.com/Purfview/whisper-standalone-win/releases/download/faster-whisper/Whisper-Faster_r192.3_windows.zip", 87_654_143, "8150ad257fd8e46d817bb7e667260c2ce4c493d9e58973862e6409c592b44ba5", Extract::ZipFlat, "tools/whisper", "tools/whisper/whisper-faster.exe"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3/model.bin", "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/edaa852ec7e145841d8ffdb056a99866b5f0a478/model.bin", 3_087_284_237, "69f74147e3334731bc3a76048724833325d2ec74642fb52620eda87352e3d4f1"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3/config.json", "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/edaa852ec7e145841d8ffdb056a99866b5f0a478/config.json", 2_394, "a9306624f5ec14270a014b647e5c316b6e03a662c369758d1b90697a7b0655b9"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3/preprocessor_config.json", "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/edaa852ec7e145841d8ffdb056a99866b5f0a478/preprocessor_config.json", 340, "7ccc62c6f2765af1f3b46c00c9b5894426835a05021c8b9c01eecb6dfb542711"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3/tokenizer.json", "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/edaa852ec7e145841d8ffdb056a99866b5f0a478/tokenizer.json", 2_480_617, "6d8cbd7cd0d8d5815e478dac67b85a26bbe77c1f5e0c6d76d1ce2abc0e5f21ca"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3/vocabulary.json", "https://huggingface.co/Systran/faster-whisper-large-v3/resolve/edaa852ec7e145841d8ffdb056a99866b5f0a478/vocabulary.json", 1_068_114, "c69260f2ab26d659b7c398f9a2b2b48ed0df16c3b47d7326782fd9cba71690c1"),
            ],
        },
        Component {
            id: "whisper-large-v3-turbo",
            title: "Распознавание речи: Whisper large-v3 turbo",
            note: "почти как large-v3, но в разы быстрее; понимает почти все языки мира",
            requirement: Requirement::Optional,
            variant_of: "asr",
            files: &[
                FileSpec::archive("downloads/whisper.zip", "https://github.com/Purfview/whisper-standalone-win/releases/download/faster-whisper/Whisper-Faster_r192.3_windows.zip", 87_654_143, "8150ad257fd8e46d817bb7e667260c2ce4c493d9e58973862e6409c592b44ba5", Extract::ZipFlat, "tools/whisper", "tools/whisper/whisper-faster.exe"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3-turbo/model.bin", "https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2/resolve/4df90f75321148c3a29a9e2351b7ddf8f5b115a8/model.bin", 1_617_884_929, "e76620f83d5f5b69efd3d87e3dc180c1bd21df9fbebacfd4335e5e1efcc018da"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3-turbo/config.json", "https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2/resolve/4df90f75321148c3a29a9e2351b7ddf8f5b115a8/config.json", 2_263, "b0253ea6c0d3bea6b1e19e91a02acfd3b53f4467362efcb5a3e6b16c9b3a9b7e"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3-turbo/preprocessor_config.json", "https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2/resolve/4df90f75321148c3a29a9e2351b7ddf8f5b115a8/preprocessor_config.json", 340, "7ccc62c6f2765af1f3b46c00c9b5894426835a05021c8b9c01eecb6dfb542711"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3-turbo/tokenizer.json", "https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2/resolve/4df90f75321148c3a29a9e2351b7ddf8f5b115a8/tokenizer.json", 2_710_337, "297b13372ac43916285644fb9687add3cc62ee2a1adb60da3dc25cc94c1871fd"),
                FileSpec::plain("models/whisper/faster-whisper-large-v3-turbo/vocabulary.json", "https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2/resolve/4df90f75321148c3a29a9e2351b7ddf8f5b115a8/vocabulary.json", 1_068_114, "c69260f2ab26d659b7c398f9a2b2b48ed0df16c3b47d7326782fd9cba71690c1"),
            ],
        },
        Component {
            id: "whisper-cuda",
            title: "Whisper на видеокарте",
            note: "библиотеки CUDA 11 для Whisper; без них Whisper считает только на процессоре",
            requirement: Requirement::Optional,
            variant_of: "",
            files: &[
                FileSpec::archive("downloads/whisper-cublas.zip", "https://developer.download.nvidia.com/compute/cuda/redist/libcublas/windows-x86_64/libcublas-windows-x86_64-11.11.3.6-archive.zip", 420_850_025, "67b0934a6359e4ee26fff823c356021589d392c4fd49ca12624f570edc08e2b9", Extract::ZipDlls, "tools/whisper", "tools/whisper/cublas64_11.dll"),
                FileSpec::archive("downloads/whisper-cudnn.zip", "https://developer.download.nvidia.com/compute/cudnn/redist/cudnn/windows-x86_64/cudnn-windows-x86_64-8.9.7.29_cuda11-archive.zip", 704_240_064, "5e45478efe71a96329e6c0d2a3a2f79c747c15b2a51fead4b84c89b02cbf1671", Extract::ZipDlls, "tools/whisper", "tools/whisper/cudnn64_8.dll"),
            ],
        },
    ];

pub fn manifest() -> Vec<Component> {
    COMPONENTS.to_vec()
}

/// Какие настройки распознавания включает вариант: движок и его модель. Скачанный вариант сразу
/// становится активным — иначе выбор на первом запуске ничего бы не менял.
pub fn variant_selection(id: &str) -> Option<(&'static str, String)> {
    match id {
        "asr" => Some(("parakeet", "int8".into())),
        "asr-ultra" => Some(("parakeet", "ultra".into())),
        other => other.strip_prefix("whisper-").filter(|model| WHISPER_MODELS.contains(model)).map(|model| ("whisper", model.to_string())),
    }
}

/// Модели Whisper в порядке показа.
pub const WHISPER_MODELS: [&str; 6] = ["large-v3-turbo", "large-v3", "medium", "small", "base", "tiny"];

/// Записать выбор варианта в настройки.
pub fn select_variant(runtime: &mut crate::runtime::Runtime, id: &str) -> bool {
    let Some((engine, model)) = variant_selection(id) else { return false };
    runtime.asr_engine = engine.to_string();
    if engine == "whisper" {
        runtime.whisper_model = model;
    } else {
        runtime.asr_model = model;
    }
    true
}

/// Этот ли вариант сейчас выбран.
pub fn variant_selected(runtime: &crate::runtime::Runtime, id: &str) -> bool {
    match variant_selection(id) {
        Some(("whisper", model)) => runtime.asr_engine == "whisper" && runtime.whisper_model == model,
        Some((_, model)) => runtime.asr_engine != "whisper" && runtime.asr_model == model,
        None => false,
    }
}

/// Каталог весов выбранного варианта Parakeet.
pub fn parakeet_dir(root: &Path, runtime: &crate::runtime::Runtime) -> PathBuf {
    let models = root.join("models");
    if runtime.asr_model == "ultra" { models.join("asr-ultra") } else { models.join("asr") }
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
    /// Программа (yue2, dub...), у которой компонент взят в этом запуске вместо закачки.
    pub taken_from: Option<String>,
}

/// Что идёт по сети прямо сейчас и чем сорвались прошлые закачки, по id компонента. Байты на
/// диске не скажут ни того, ни другого: сервер без докачки пишет файл без манифеста частей.
#[derive(Default)]
struct Downloads {
    live: std::collections::HashMap<&'static str, u64>,
    failed: std::collections::HashMap<&'static str, String>,
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
                match studio_hub_client::neighbors::take_file(&source, &target, false, |_| true) {
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
            ComponentStatus { component, present, have_bytes, selected, error, taken_from }
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
    for status in statuses.iter_mut() {
        if status.component.requirement != Requirement::Optional && elsewhere(status.component.id) {
            status.component.requirement = Requirement::Optional;
        }
    }
    installed_variant_covers_group(&mut statuses);
    statuses
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
        downloads.failed.insert(id, error.clone());
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
    let (total, ranged) = probe_size(&agent, spec.url);
    if total == 0 {
        return Err(format!("не удалось узнать размер {title}"));
    }

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
