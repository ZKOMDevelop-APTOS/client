use crate::consts::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;
use sysinfo::System;

#[derive(Debug, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu_serial: String,
    pub gpu_uuid: Option<String>,
    pub system_fingerprint: String,
    pub gpu_model: Option<String>,
    pub gpu_memory: Option<u64>,
    pub cuda_version: Option<String>,
    pub driver_version: Option<String>,
}

pub struct HardwareCollector {
    sys: System,
}

impl HardwareCollector {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
        }
    }

    pub fn collect_info(&self) -> Result<HardwareInfo> {
        Ok(HardwareInfo {
            cpu_serial: self.get_cpu_serial()?,
            gpu_uuid: self.get_gpu_uuid(),
            system_fingerprint: self.generate_system_fingerprint()?,
            gpu_model: self.get_gpu_model(),
            gpu_memory: self.get_gpu_memory(),
            cuda_version: self.get_cuda_version(),
            driver_version: self.get_driver_version(),
        })
    }

    fn get_cpu_serial(&self) -> Result<String> {
        // 在 Linux 系统上获取 CPU 序列号
        let output = Command::new("cat").arg("/proc/cpuinfo").output()?;

        let cpu_info = String::from_utf8(output.stdout)?;
        let serial = cpu_info
            .lines()
            .find(|line| line.starts_with("Serial"))
            .and_then(|line| line.split(':').nth(1))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(serial)
    }

    fn get_gpu_uuid(&self) -> Option<String> {
        // 尝试获取 NVIDIA GPU UUID
        if let Ok(output) = Command::new("nvidia-smi")
            .arg("--query-gpu=gpu_uuid")
            .arg("--format=csv,noheader")
            .output()
        {
            if let Ok(uuid) = String::from_utf8(output.stdout) {
                return Some(uuid.trim().to_string());
            }
        }
        None
    }

    fn get_gpu_model(&self) -> Option<String> {
        if let Ok(output) = Command::new("nvidia-smi")
            .arg("--query-gpu=gpu_name")
            .arg("--format=csv,noheader")
            .output()
        {
            if let Ok(model) = String::from_utf8(output.stdout) {
                return Some(model.trim().to_string());
            }
        }
        None
    }

    fn get_gpu_memory(&self) -> Option<u64> {
        if let Ok(output) = Command::new("nvidia-smi")
            .arg("--query-gpu=memory.total")
            .arg("--format=csv,noheader")
            .output()
        {
            if let Ok(memory) = String::from_utf8(output.stdout) {
                if let Ok(memory_mb) = memory.trim().parse::<u64>() {
                    return Some(memory_mb);
                }
            }
        }
        None
    }

    fn get_cuda_version(&self) -> Option<String> {
        if let Ok(output) = Command::new("nvcc").arg("--version").output() {
            if let Ok(version) = String::from_utf8(output.stdout) {
                // Extract CUDA version from nvcc output
                if let Some(line) = version.lines().find(|line| line.contains("release")) {
                    if let Some(version) = line.split_whitespace().nth(5) {
                        return Some(version.to_string());
                    }
                }
            }
        }
        None
    }

    fn get_driver_version(&self) -> Option<String> {
        if let Ok(output) = Command::new("nvidia-smi")
            .arg("--query-gpu=driver_version")
            .arg("--format=csv,noheader")
            .output()
        {
            if let Ok(version) = String::from_utf8(output.stdout) {
                return Some(version.trim().to_string());
            }
        }
        None
    }

    fn generate_system_fingerprint(&self) -> Result<String> {
        // 收集系统信息生成指纹
        let mut fingerprint = String::new();

        // 添加 CPU 信息
        let cpu = &self.sys.cpus()[0];
        fingerprint.push_str(&format!(
            "{}{}{}",
            FINGERPRINT_CPU_PREFIX,
            FINGERPRINT_SEPARATOR,
            cpu.brand()
        ));

        // 添加内存信息
        let total_memory = self.sys.total_memory();
        fingerprint.push_str(&format!(
            "{}{}{}{}",
            FINGERPRINT_SEPARATOR, FINGERPRINT_MEM_PREFIX, FINGERPRINT_SEPARATOR, total_memory
        ));

        // 添加系统信息
        fingerprint.push_str(&format!(
            "{}{}{}{}",
            FINGERPRINT_SEPARATOR,
            FINGERPRINT_OS_PREFIX,
            FINGERPRINT_SEPARATOR,
            std::env::consts::OS
        ));

        Ok(fingerprint)
    }
}
