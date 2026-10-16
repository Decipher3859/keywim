//! Battery readings used by monitoring and notification decisions.
use std::{
    fs::{read_dir, read_to_string},
    io::{Error, ErrorKind, Result},
    path::{Path, PathBuf},
};

pub struct BatteryStatus {
    pub percent: u8,
    pub discharging: bool,
}

impl BatteryStatus {
    pub fn read(path: &Path) -> Result<Self> {
        let capacity = read_to_string(path.join("capacity"))?;
        let percent = capacity
            .trim()
            .parse::<u8>()
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

        // Reject invalid readings before using them for warning thresholds.
        if percent > 100 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Battery percentage exceeds 100",
            ));
        }
        let status = read_to_string(path.join("status"))?;
        Ok(Self {
            percent,
            discharging: status.trim() == "Discharging",
        })
    }
}

pub fn find_battery() -> Result<Option<PathBuf>> {
    // Detect the battery by its type.
    for entry in read_dir("/sys/class/power_supply")? {
        let path = entry?.path();
        let supply_type = read_to_string(path.join("type"))?;
        if supply_type.trim() == "Battery" {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

#[derive(Default)]
pub struct BatteryWarning {
    warned: bool,
}

impl BatteryWarning {
    pub fn update(&mut self, status: &BatteryStatus) -> bool {
        // Rearm after charging start or the battery leaves the low range.
        if !status.discharging || status.percent > 20 {
            self.warned = false;
            return false;
        }

        // Emit one warning envent per uninterrupted low-battery period.
        if self.warned {
            return false;
        }
        self.warned = true;
        true
    }
}
