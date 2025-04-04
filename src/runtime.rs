use anyhow::{Result, Context};
use std::process::Command;

pub struct RuntimeChecker;

impl RuntimeChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check_environment(&self) -> Result<()> {
        self.check_cuda()?;
        self.check_docker()?;
        Ok(())
    }

    fn check_cuda(&self) -> Result<()> {
        // Check if nvidia-smi is available
        let output = Command::new("nvidia-smi")
            .output()
            .context("Failed to execute nvidia-smi. CUDA environment may not be properly set up.")?;

        if !output.status.success() {
            anyhow::bail!("CUDA environment check failed. Please ensure CUDA is properly installed.");
        }

        log::info!("CUDA environment check passed");
        Ok(())
    }

    fn check_docker(&self) -> Result<()> {
        // Check if docker daemon is running
        let output = Command::new("docker")
            .arg("info")
            .output()
            .context("Failed to execute docker. Docker may not be installed or running.")?;

        if !output.status.success() {
            anyhow::bail!("Docker environment check failed. Please ensure Docker is installed and running.");
        }

        log::info!("Docker environment check passed");
        Ok(())
    }
} 