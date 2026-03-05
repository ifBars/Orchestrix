use std::process::Command;

use ort::ep::{CoreML, DirectML, ExecutionProvider, CUDA};
use serde::Serialize;
use serde_json::Value;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: Option<String>,
    pub vram_total_bytes: Option<u64>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrtGpuBackends {
    pub cuda: bool,
    pub directml: bool,
    pub coreml: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardwareProfile {
    pub ram_bytes: u64,
    pub logical_cores: usize,
    pub physical_cores: usize,
    pub gpus: Vec<GpuInfo>,
    pub ort_backends: OrtGpuBackends,
}

pub fn detect_hardware_profile() -> HardwareProfile {
    let mut system = System::new_all();
    system.refresh_all();

    let ram_bytes = system.total_memory();
    let logical_cores = system.cpus().len();
    let physical_cores = system.physical_core_count().unwrap_or(logical_cores.max(1));

    let mut gpus = Vec::new();
    gpus.extend(detect_windows_gpus());
    gpus.extend(detect_macos_gpus());
    gpus.extend(detect_linux_gpus());
    dedupe_gpus(&mut gpus);

    let ort_backends = OrtGpuBackends {
        cuda: CUDA::default().is_available().unwrap_or(false),
        directml: DirectML::default().is_available().unwrap_or(false),
        coreml: CoreML::default().is_available().unwrap_or(false),
    };

    HardwareProfile {
        ram_bytes,
        logical_cores,
        physical_cores,
        gpus,
        ort_backends,
    }
}

fn detect_windows_gpus() -> Vec<GpuInfo> {
    if !cfg!(target_os = "windows") {
        return Vec::new();
    }

    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg("Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Json -Compress")
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let parsed = serde_json::from_slice::<Value>(&output.stdout).ok();
    let Some(parsed) = parsed else {
        return Vec::new();
    };

    match parsed {
        Value::Array(items) => items.into_iter().filter_map(parse_windows_gpu).collect(),
        Value::Object(_) => parse_windows_gpu(parsed).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn parse_windows_gpu(value: Value) -> Option<GpuInfo> {
    let name = value.get("Name")?.as_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }

    let vram_total_bytes = value
        .get("AdapterRAM")
        .and_then(Value::as_u64)
        .filter(|bytes| *bytes > 0);

    Some(GpuInfo {
        vendor: infer_vendor(&name),
        name,
        vram_total_bytes,
        source: "win32_video_controller".to_string(),
    })
}

fn detect_macos_gpus() -> Vec<GpuInfo> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }

    let output = Command::new("system_profiler")
        .arg("SPDisplaysDataType")
        .arg("-json")
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let parsed = serde_json::from_slice::<Value>(&output.stdout).ok();
    let Some(parsed) = parsed else {
        return Vec::new();
    };

    parsed
        .get("SPDisplaysDataType")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item
                        .get("sppci_model")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    if name.is_empty() {
                        return None;
                    }

                    let vram_total_bytes = item
                        .get("spdisplays_vram")
                        .and_then(Value::as_str)
                        .and_then(parse_human_size_bytes)
                        .or_else(|| {
                            item.get("spdisplays_vram_shared")
                                .and_then(Value::as_str)
                                .and_then(parse_human_size_bytes)
                        });

                    Some(GpuInfo {
                        vendor: infer_vendor(&name),
                        name,
                        vram_total_bytes,
                        source: "system_profiler".to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn detect_linux_gpus() -> Vec<GpuInfo> {
    if !cfg!(target_os = "linux") {
        return Vec::new();
    }

    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=name,memory.total")
        .arg("--format=csv,noheader,nounits")
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(',').map(str::trim);
            let name = parts.next().unwrap_or_default().to_string();
            if name.is_empty() {
                return None;
            }

            let mib = parts.next().and_then(|value| value.parse::<u64>().ok());
            let vram_total_bytes = mib.map(|value| value * 1024 * 1024);

            Some(GpuInfo {
                vendor: infer_vendor(&name),
                name,
                vram_total_bytes,
                source: "nvidia_smi".to_string(),
            })
        })
        .collect()
}

fn parse_human_size_bytes(raw: &str) -> Option<u64> {
    let cleaned = raw.replace("GB", " GB").replace("MB", " MB");
    let mut parts = cleaned.split_whitespace();
    let value = parts.next()?.parse::<f64>().ok()?;
    let unit = parts.next()?.to_ascii_lowercase();

    let bytes = match unit.as_str() {
        "gb" => value * 1024.0 * 1024.0 * 1024.0,
        "mb" => value * 1024.0 * 1024.0,
        _ => return None,
    };

    Some(bytes as u64)
}

fn infer_vendor(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("nvidia") {
        return Some("nvidia".to_string());
    }
    if lower.contains("amd") || lower.contains("radeon") {
        return Some("amd".to_string());
    }
    if lower.contains("intel") {
        return Some("intel".to_string());
    }
    if lower.contains("apple") {
        return Some("apple".to_string());
    }
    None
}

fn dedupe_gpus(gpus: &mut Vec<GpuInfo>) {
    let mut seen = std::collections::HashSet::new();
    gpus.retain(|gpu| seen.insert(gpu.name.to_ascii_lowercase()));
}
