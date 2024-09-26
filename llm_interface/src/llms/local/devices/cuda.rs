use super::gpu::GpuDevice;
use nvml_wrapper::Nvml;

// See https://gist.github.com/jrruethe/8974d2c8b4ece242a071d1a1526aa763#file-vram-rb-L64
pub const CUDA_OVERHEAD: u64 = 500 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct CudaDeviceMap {
    /// The main GPU device ordinal. Defaults to the largest VRAM device.
    pub main_gpu: Option<u32>,
    /// Ordinals of the devices to use.
    pub use_cuda_devices: Vec<u32>,
    pub(crate) cuda_devices: Vec<CudaDevice>,
    pub(crate) total_vram_bytes: u64,
    pub(crate) error_on_gpu_error: bool,
}

impl Default for CudaDeviceMap {
    fn default() -> Self {
        Self {
            main_gpu: None,
            use_cuda_devices: Vec::new(),
            cuda_devices: Vec::new(),
            total_vram_bytes: 0,
            error_on_gpu_error: true,
        }
    }
}

impl CudaDeviceMap {
    pub fn new(use_cuda_devices: Vec<u32>, main_gpu: Option<u32>) -> Self {
        Self {
            main_gpu,
            use_cuda_devices,
            ..Default::default()
        }
    }

    pub(crate) fn initialize(&mut self) -> crate::Result<()> {
        let nvml: Nvml = init_nvml_wrapper()?;
        if self.use_cuda_devices.is_empty() {
            self.cuda_devices = get_all_cuda_devices(Some(&nvml))?;
        } else {
            for ordinal in &self.use_cuda_devices {
                match CudaDevice::new(*ordinal, Some(&nvml)) {
                    Ok(cuda_device) => self.cuda_devices.push(cuda_device),
                    Err(e) => {
                        crate::warn!("Failed to get device {}: {}", ordinal, e);
                        if self.error_on_gpu_error {
                            crate::bail!("Failed to get device {}: {}", ordinal, e);
                        }
                    }
                }
            }
        }
        if self.cuda_devices.is_empty() {
            crate::bail!("No CUDA devices found");
        }

        self.main_gpu = Some(self.main_gpu()?);

        self.total_vram_bytes = self
            .cuda_devices
            .iter()
            .map(|d| (d.available_vram_bytes))
            .sum();
        Ok(())
    }

    pub(crate) fn device_count(&self) -> usize {
        self.cuda_devices.len()
    }

    pub(crate) fn main_gpu(&self) -> crate::Result<u32> {
        if let Some(main_gpu) = self.main_gpu {
            for device in &self.cuda_devices {
                if device.ordinal == main_gpu {
                    return Ok(main_gpu);
                }
            }
            if self.error_on_gpu_error {
                crate::bail!(
                    "Main GPU set by user {} not found in CUDA devices",
                    main_gpu
                );
            }
        };
        let main_gpu = self
            .cuda_devices
            .iter()
            .max_by_key(|d| d.available_vram_bytes)
            .ok_or_else(|| crate::anyhow!("No devices found when setting main gpu"))?
            .ordinal;
        for device in &self.cuda_devices {
            if device.ordinal == main_gpu {
                return Ok(main_gpu);
            }
        }
        crate::bail!("Main GPU {} not found in CUDA devices", main_gpu);
    }

    pub(crate) fn to_generic_gpu_devices(&self) -> crate::Result<Vec<GpuDevice>> {
        let mut gpu_devices: Vec<GpuDevice> = self
            .cuda_devices
            .iter()
            .map(|d| d.to_generic_gpu())
            .collect();
        let main_gpu = self.main_gpu()?;
        for gpu in &mut gpu_devices {
            if gpu.ordinal == main_gpu {
                gpu.is_main_gpu = true;
            }
        }
        Ok(gpu_devices)
    }
}

pub fn get_all_cuda_devices(nvml: Option<&Nvml>) -> crate::Result<Vec<CudaDevice>> {
    let nvml = match nvml {
        Some(nvml) => nvml,
        None => &init_nvml_wrapper()?,
    };
    let device_count = nvml.device_count()?;
    let mut cuda_devices: Vec<CudaDevice> = Vec::new();
    let mut ordinal = 0;
    while cuda_devices.len() < device_count as usize {
        if let Ok(nvml_device) = CudaDevice::new(ordinal, Some(&nvml)) {
            cuda_devices.push(nvml_device);
        }
        if ordinal > 100 {
            crate::warn!(
                "nvml_wrapper reported {device_count} devices, but we were only able to get {}",
                cuda_devices.len()
            );
        }
        ordinal += 1;
    }
    for d in cuda_devices.iter() {
        crate::info!(
            "Device {}: {:.2} Gigabytes",
            d.ordinal,
            (d.available_vram_bytes as f64) / 1_073_741_824.0
        );
    }
    if cuda_devices.len() == 0 {
        crate::bail!("No CUDA devices found");
    }
    Ok(cuda_devices)
}

#[derive(Debug, Clone)]
pub struct CudaDevice {
    pub ordinal: u32,
    pub available_vram_bytes: u64,
    pub name: Option<String>,
    pub power_limit: Option<u32>,
    pub driver_major: Option<i32>,
    pub driver_minor: Option<i32>,
}

impl CudaDevice {
    pub fn new(ordinal: u32, nvml: Option<&Nvml>) -> crate::Result<Self> {
        let nvml = match nvml {
            Some(nvml) => nvml,
            None => &init_nvml_wrapper()?,
        };
        if let Ok(nvml_device) = nvml.device_by_index(ordinal) {
            if let Ok(memory_info) = nvml_device.memory_info() {
                if memory_info.total != 0 {
                    let name = if let Ok(name) = nvml_device.name() {
                        Some(name)
                    } else {
                        None
                    };
                    let power_limit = if let Ok(power_limit) = nvml_device.enforced_power_limit() {
                        Some(power_limit)
                    } else {
                        None
                    };
                    let (driver_major, driver_minor) = if let Ok(cuda_compute_capability) =
                        nvml_device.cuda_compute_capability()
                    {
                        (
                            Some(cuda_compute_capability.major),
                            Some(cuda_compute_capability.minor),
                        )
                    } else {
                        (None, None)
                    };
                    let cuda_device = CudaDevice {
                        ordinal: ordinal,
                        available_vram_bytes: memory_info.total - CUDA_OVERHEAD,
                        name,
                        power_limit,
                        driver_major,
                        driver_minor,
                    };

                    crate::info!(?cuda_device);
                    Ok(cuda_device)
                } else {
                    crate::bail!("Device {} has 0 bytes of VRAM. Skipping device.", ordinal);
                }
            } else {
                crate::bail!("Failed to get device {}", ordinal);
            }
        } else {
            crate::bail!("Failed to get device {}", ordinal);
        }
    }

    pub fn to_generic_gpu(&self) -> GpuDevice {
        GpuDevice {
            ordinal: self.ordinal,
            available_vram_bytes: self.available_vram_bytes,
            allocated_bytes: 0,
            allocated_buffer_bytes: 0,
            allocated_layers: 0,
            is_main_gpu: false,
        }
    }
}

pub(crate) fn init_nvml_wrapper() -> crate::Result<Nvml> {
    let library_names = vec![
        "libnvidia-ml.so",   // For Linux
        "libnvidia-ml.so.1", // For WSL
        "nvml.dll",          // For Windows
    ];
    for library_name in library_names {
        match Nvml::builder().lib_path(library_name.as_ref()).init() {
            Ok(nvml) => return Ok(nvml),
            Err(_) => {
                continue;
            }
        }
    }
    crate::bail!("Failed to initialize nvml_wrapper::Nvml")
}
