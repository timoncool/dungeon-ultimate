//! Монитор ресурсов: снимок GPU (NVML) + RAM (sysinfo). Отдаётся по GET /hw/snapshot, фронт опрашивает
//! раз в секунду. NVML/System держим в ленивых статиках — не тащим в AppState.

use std::sync::{Mutex, OnceLock};

use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use serde::Serialize;
use sysinfo::{Pid, System};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSnapshot {
    pub gpu_name: String,
    pub total_vram: u64,
    pub used_vram: u64,
    pub free_vram: u64,
    pub gpu_utilization: f64,
    pub temperature: f64,
    pub power_draw: f64,
    pub power_limit: f64,
    pub process_ram: u64,
    pub total_ram: u64,
    pub used_ram: u64,
    pub message: String,
}

fn nvml() -> &'static Mutex<Option<Nvml>> {
    static N: OnceLock<Mutex<Option<Nvml>>> = OnceLock::new();
    N.get_or_init(|| Mutex::new(None))
}

fn sysinfo() -> &'static Mutex<System> {
    static S: OnceLock<Mutex<System>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(System::new()))
}

pub fn snapshot() -> HardwareSnapshot {
    let mut snap = HardwareSnapshot::default();

    // GPU через NVML (ленивая инициализация на первом опросе).
    let mut ng = nvml().lock().unwrap();
    if ng.is_none() {
        match Nvml::init() {
            Ok(n) => *ng = Some(n),
            Err(e) => snap.message = format!("NVML init: {e}"),
        }
    }
    if let Some(n) = ng.as_ref() {
        match n.device_by_index(0) {
            Ok(d) => {
                snap.gpu_name = d.name().unwrap_or_else(|_| "NVIDIA GPU".into());
                if let Ok(m) = d.memory_info() {
                    snap.total_vram = m.total;
                    snap.used_vram = m.used;
                    snap.free_vram = m.free;
                }
                if let Ok(u) = d.utilization_rates() {
                    snap.gpu_utilization = u.gpu as f64;
                }
                if let Ok(t) = d.temperature(TemperatureSensor::Gpu) {
                    snap.temperature = t as f64;
                }
                if let Ok(p) = d.power_usage() {
                    snap.power_draw = p as f64 / 1000.0;
                }
                if let Ok(l) = d.power_management_limit() {
                    snap.power_limit = l as f64 / 1000.0;
                }
            }
            Err(e) => snap.message = format!("NVML device: {e}"),
        }
    } else if snap.message.is_empty() {
        snap.message = "нет NVIDIA GPU".into();
    }
    drop(ng);

    // RAM системы + процесса через sysinfo.
    let mut sg = sysinfo().lock().unwrap();
    sg.refresh_memory();
    snap.total_ram = sg.total_memory();
    snap.used_ram = sg.used_memory();
    let pid = Pid::from(std::process::id() as usize);
    sg.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    if let Some(p) = sg.process(pid) {
        snap.process_ram = p.memory();
    }

    snap
}
