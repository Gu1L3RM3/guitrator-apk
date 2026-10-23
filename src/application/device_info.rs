use crate::domain::device_stats::{parse_df, parse_meminfo};
use crate::domain::{AdbCommandExecutor, AdbError, DeviceInfo};

/// Use case: summarise the connected phone (model, Android version, RAM and storage).
pub struct ReadDeviceInfo<'a> {
    executor: &'a dyn AdbCommandExecutor,
}

impl<'a> ReadDeviceInfo<'a> {
    pub fn new(executor: &'a dyn AdbCommandExecutor) -> Self {
        Self { executor }
    }

    /// Fails only when the phone cannot be queried at all. Memory and storage are best effort.
    pub fn execute(&self) -> Result<DeviceInfo, AdbError> {
        let model = self.getprop("ro.product.model")?;
        let android_version = self.getprop("ro.build.version.release")?;

        let (ram_total_kb, ram_available_kb) = self
            .executor
            .run(&["shell", "cat /proc/meminfo"])
            .map(|out| parse_meminfo(&out))
            .unwrap_or((None, None));

        let (storage_total_kb, storage_free_kb) = self
            .executor
            .run(&["shell", "df -k /data 2>/dev/null; true"])
            .map(|out| parse_df(&out))
            .unwrap_or((None, None));

        Ok(DeviceInfo {
            model,
            android_version,
            ram_total_kb,
            ram_available_kb,
            storage_total_kb,
            storage_free_kb,
        })
    }

    fn getprop(&self, key: &str) -> Result<String, AdbError> {
        let value = self.executor.run(&["shell", &format!("getprop {key}")])?;
        Ok(value.trim().to_string())
    }
}
