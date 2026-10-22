//! На чём игра считает: CUDA 13, CUDA 12, Vulkan или процессор. Решает карта и её драйвер — те же пороги, что у
//! YuE2 Studio (`music-server/src/hardware.rs`): CUDA 13 — Turing и новее с драйвером от 580, CUDA 12 — Maxwell,
//! Pascal, Volta и любая карта со старым драйвером от 525, иначе Vulkan на любой карте, а без карты — процессор.

use std::sync::OnceLock;

use serde::Serialize;

/// Чем считать на этой машине.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Accel {
    Cuda13,
    Cuda12,
    Vulkan,
    Cpu,
}

impl Accel {
    pub fn key(self) -> &'static str {
        match self {
            Accel::Cuda13 => "cuda13",
            Accel::Cuda12 => "cuda12",
            Accel::Vulkan => "vulkan",
            Accel::Cpu => "cpu",
        }
    }

    pub fn nvidia(self) -> bool {
        matches!(self, Accel::Cuda13 | Accel::Cuda12)
    }
}

/// Первый драйвер каждой линии CUDA на Windows (таблицы совместимости NVIDIA).
pub const CUDA13_DRIVER: u32 = 580;
const CUDA12_DRIVER: u32 = 525;
/// Самая старая архитектура сборки CUDA 12 — Maxwell 5.2; CUDA 13 начинается с Turing 7.5.
const CUDA12_OLDEST: (u32, u32) = (5, 2);
const CUDA13_OLDEST: (u32, u32) = (7, 5);

/// Сборка CUDA по вычислительной способности карты и старшему номеру драйвера.
pub fn cuda_build(compute: (u32, u32), driver_major: u32) -> Option<Accel> {
    if compute >= CUDA13_OLDEST && driver_major >= CUDA13_DRIVER {
        Some(Accel::Cuda13)
    } else if compute >= CUDA12_OLDEST && driver_major >= CUDA12_DRIVER {
        Some(Accel::Cuda12)
    } else {
        None
    }
}

/// Карта NVIDIA: номер в порядке CUDA (по шине PCI), имя, память, вычислительная способность.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub index: u32,
    pub name: String,
    pub vram_gb: f64,
    pub compute: (u32, u32),
}

/// Что стоит в машине.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub nvidia: Vec<Card>,
    pub driver_major: Option<u32>,
    /// Видеокарта не NVIDIA (AMD, Intel): на ней считает Vulkan.
    pub other_adapter: Option<String>,
}

/// Железо не меняется за время жизни процесса, а экран закачки спрашивает статус раз в секунду: опрос один.
pub fn machine() -> &'static Machine {
    static MACHINE: OnceLock<Machine> = OnceLock::new();
    MACHINE.get_or_init(|| {
        let mut machine = Machine::default();
        if let Ok(nvml) = nvml_wrapper::Nvml::init() {
            machine.driver_major = nvml
                .sys_driver_version()
                .ok()
                .and_then(|version| version.split('.').next().and_then(|major| major.trim().parse().ok()));
            for index in 0..nvml.device_count().unwrap_or(0) {
                let Ok(device) = nvml.device_by_index(index) else { continue };
                let Ok(compute) = device.cuda_compute_capability() else { continue };
                machine.nvidia.push(Card {
                    index,
                    name: device.name().unwrap_or_else(|_| "NVIDIA".into()),
                    vram_gb: device.memory_info().map(|memory| memory.total as f64 / (1u64 << 30) as f64).unwrap_or(0.0),
                    compute: (compute.major as u32, compute.minor as u32),
                });
            }
        }
        machine.other_adapter = display_adapter().filter(|name| !name.to_ascii_lowercase().contains("nvidia"));
        machine
    })
}

/// Карта, выбранная в настройках (номер CUDA), или первая.
pub fn chosen_card(runtime: &crate::runtime::Runtime) -> Option<&'static Card> {
    let cards = &machine().nvidia;
    runtime.gpu_index.and_then(|index| cards.iter().find(|card| card.index == index)).or_else(|| cards.first())
}

/// Карта из настроек, которой нет в компьютере: с ней в `CUDA_VISIBLE_DEVICES` CUDA не видит ни одного устройства, и
/// движок без слова ушёл бы на процессор. Спрашивают те, кто идёт на карту через CUDA.
pub fn missing_chosen_card(runtime: &crate::runtime::Runtime) -> Option<crate::errors::UserError> {
    missing_card(runtime.gpu_index, &machine().nvidia)
}

fn missing_card(chosen: Option<u32>, cards: &[Card]) -> Option<crate::errors::UserError> {
    let index = chosen?;
    if cards.iter().any(|card| card.index == index) {
        return None;
    }
    let cards = cards.iter().map(|card| format!("{}: {}", card.index, card.name)).collect();
    Some(crate::errors::UserError::GpuMissing { index, cards })
}

/// Чем считать на этой машине. `DU_ACCEL` (cuda13 | cuda12 | vulkan | cpu) задаёт это руками — проверка сборок под
/// чужое железо на своей машине.
pub fn accel(runtime: &crate::runtime::Runtime) -> Accel {
    if let Ok(forced) = std::env::var("DU_ACCEL") {
        match forced.trim() {
            "cuda13" => return Accel::Cuda13,
            "cuda12" => return Accel::Cuda12,
            "vulkan" => return Accel::Vulkan,
            "cpu" => return Accel::Cpu,
            other => tracing::warn!("DU_ACCEL={other} не понят: cuda13, cuda12, vulkan или cpu"),
        }
    }
    let machine = machine();
    if let (Some(card), Some(driver)) = (chosen_card(runtime), machine.driver_major) {
        if let Some(build) = cuda_build(card.compute, driver) {
            return build;
        }
    }
    if !machine.nvidia.is_empty() || machine.other_adapter.is_some() {
        Accel::Vulkan
    } else {
        Accel::Cpu
    }
}

/// Выбранная в настройках карта — до первого обращения к CUDA: и движки в процессе игры, и llama-server читают
/// `CUDA_VISIBLE_DEVICES` один раз. Порядок карт — по шине PCI, как их нумерует NVML. В журнал — что найдено и чем
/// игра будет считать.
pub fn apply_saved_gpu(root: &std::path::Path) {
    let runtime = crate::runtime::load(root);
    if let Some(index) = runtime.gpu_index {
        if std::env::var_os("CUDA_VISIBLE_DEVICES").is_none() {
            std::env::set_var("CUDA_DEVICE_ORDER", "PCI_BUS_ID");
            std::env::set_var("CUDA_VISIBLE_DEVICES", index.to_string());
        }
    }
    let machine = machine();
    let cards: Vec<String> = machine
        .nvidia
        .iter()
        .map(|card| format!("{}: {} ({:.0} ГБ, CC {}.{})", card.index, card.name, card.vram_gb, card.compute.0, card.compute.1))
        .collect();
    tracing::info!(
        "железо: NVIDIA [{}], драйвер {:?}, другая карта {:?}; считаем на {}",
        cards.join("; "),
        machine.driver_major,
        machine.other_adapter,
        accel(&runtime).key()
    );
    if let Some(problem) = missing_chosen_card(&runtime) {
        tracing::warn!("{problem}");
    }
}

/// Видеокарта с наибольшей памятью из записи её драйвера: `HardwareInformation.qwMemorySize` — 64-битный размер,
/// который даёт сам драйвер (у WMI `AdapterRAM` упирается в 4 ГБ).
#[cfg(windows)]
fn display_adapter() -> Option<String> {
    const CLASS: &str = r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let query = |value: &str| -> Option<String> {
        let mut command = std::process::Command::new("reg");
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
        let output = command.args(["query", CLASS, "/s", "/v", value]).output().ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    best_adapter(&query("DriverDesc")?, &query("HardwareInformation.qwMemorySize")?).map(|(name, _)| name)
}

#[cfg(not(windows))]
fn display_adapter() -> Option<String> {
    None
}

/// Из выдачи `reg query` имён и размеров — адаптер с наибольшей памятью; «Microsoft Basic Display» не карта.
#[cfg(any(windows, test))]
fn best_adapter(names: &str, sizes: &str) -> Option<(String, f64)> {
    fn values(listing: &str) -> Vec<(String, String)> {
        let mut key = String::new();
        let mut out = Vec::new();
        for line in listing.lines() {
            if line.starts_with("HKEY_") {
                key = line.trim().to_owned();
            } else if let Some((_, value)) = line.trim().split_once("    REG_") {
                if let Some((_, data)) = value.split_once("    ") {
                    out.push((key.clone(), data.trim().to_owned()));
                }
            }
        }
        out
    }
    let names = values(names);
    values(sizes)
        .into_iter()
        .filter_map(|(key, size)| {
            let bytes = u64::from_str_radix(size.trim_start_matches("0x"), 16).ok()?;
            let name = names.iter().find(|(name_key, _)| *name_key == key)?.1.clone();
            (!name.starts_with("Microsoft")).then_some((name, bytes as f64 / 1024.0 / 1024.0 / 1024.0))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cuda_build_follows_the_architecture_and_the_driver() {
        assert_eq!(cuda_build((8, 9), 581), Some(Accel::Cuda13));
        assert_eq!(cuda_build((7, 5), 581), Some(Accel::Cuda13));
        assert_eq!(cuda_build((8, 6), 560), Some(Accel::Cuda12), "a Turing+ card with an old driver");
        assert_eq!(cuda_build((6, 1), 581), Some(Accel::Cuda12), "Pascal");
        assert_eq!(cuda_build((5, 2), 530), Some(Accel::Cuda12), "Maxwell");
        assert_eq!(cuda_build((5, 0), 581), None);
        assert_eq!(cuda_build((8, 9), 470), None);
    }

    #[test]
    fn a_chosen_card_that_is_not_there_is_said_not_left_to_the_processor() {
        let cards = vec![Card { index: 0, name: "NVIDIA GeForce RTX 4090".into(), vram_gb: 24.0, compute: (8, 9) }];
        assert!(missing_card(None, &cards).is_none());
        assert!(missing_card(Some(0), &cards).is_none());
        let missing = missing_card(Some(1), &cards).expect("карты 1 нет").to_string();
        assert!(missing.contains("номер 1") && missing.contains("0: NVIDIA GeForce RTX 4090"), "{missing}");
        assert_eq!(missing_card(Some(0), &[]).expect("карт нет совсем").code(), "gpu_missing_no_nvidia");
    }

    #[test]
    fn the_adapter_with_the_most_memory_wins_and_basic_display_never_does() {
        let names = "\r\nHKEY_LOCAL_MACHINE\\X\\0000\r\n    DriverDesc    REG_SZ    AMD Radeon RX 7800 XT\r\n\r\nHKEY_LOCAL_MACHINE\\X\\0001\r\n    DriverDesc    REG_SZ    Intel(R) UHD Graphics 770\r\n\r\nHKEY_LOCAL_MACHINE\\X\\0002\r\n    DriverDesc    REG_SZ    Microsoft Basic Display Adapter\r\n";
        let sizes = "\r\nHKEY_LOCAL_MACHINE\\X\\0000\r\n    HardwareInformation.qwMemorySize    REG_QWORD    0x400000000\r\n\r\nHKEY_LOCAL_MACHINE\\X\\0001\r\n    HardwareInformation.qwMemorySize    REG_QWORD    0x80000000\r\n\r\nHKEY_LOCAL_MACHINE\\X\\0002\r\n    HardwareInformation.qwMemorySize    REG_QWORD    0x800000000\r\n";
        let (name, vram) = best_adapter(names, sizes).unwrap();
        assert_eq!(name, "AMD Radeon RX 7800 XT");
        assert_eq!(vram, 16.0);
    }
}
