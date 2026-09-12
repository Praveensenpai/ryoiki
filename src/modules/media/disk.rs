use anyhow::{bail, Result};
use std::ffi::CString;
use std::path::Path;

/// Disk usage statistics for a given mount or path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub used_pct: u8,
}

/// Queries filesystem disk usage via `statvfs`.
pub fn get_disk_usage(path: &Path) -> Result<DiskUsage> {
    let path_str = path.to_string_lossy();
    let c_path = CString::new(path_str.as_bytes())?;

    unsafe {
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) != 0 {
            bail!("statvfs failed on path: {}", path.display());
        }
        let s = stat.assume_init();
        let bsize = s.f_frsize;
        let total_bytes = s.f_blocks.saturating_mul(bsize);
        let free_bytes = s.f_bavail.saturating_mul(bsize);
        let used_bytes = total_bytes.saturating_sub(free_bytes);

        let used_pct = used_bytes
            .saturating_mul(100)
            .checked_div(total_bytes)
            .map_or(0, |pct| u8::try_from(pct.min(100)).unwrap_or(100));

        Ok(DiskUsage {
            total_bytes,
            used_bytes,
            free_bytes,
            used_pct,
        })
    }
}

/// Calculates how many bytes must be freed to reach a target disk percentage.
#[must_use]
pub fn compute_bytes_to_free(usage: DiskUsage, target_pct: u8) -> u64 {
    if usage.used_pct <= target_pct {
        return 0;
    }
    let target_used = (usage.total_bytes.saturating_mul(u64::from(target_pct))) / 100;
    usage.used_bytes.saturating_sub(target_used)
}

/// Formats a byte count into a human-readable string (MiB or GiB).
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const BYTES_PER_MIB: u64 = 1024 * 1024;
    const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
    if bytes < BYTES_PER_GIB {
        let mib = bytes / BYTES_PER_MIB;
        let dec = (bytes % BYTES_PER_MIB) * 10 / BYTES_PER_MIB;
        format!("{mib}.{dec} MiB")
    } else {
        let gib = bytes / BYTES_PER_GIB;
        let dec = (bytes % BYTES_PER_GIB) * 10 / BYTES_PER_GIB;
        format!("{gib}.{dec} GiB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_bytes_to_free_when_under_target() {
        let usage = DiskUsage {
            total_bytes: 1000,
            used_bytes: 600,
            free_bytes: 400,
            used_pct: 60,
        };
        assert_eq!(compute_bytes_to_free(usage, 70), 0);
    }

    #[test]
    fn test_compute_bytes_to_free_when_over_target() {
        let usage = DiskUsage {
            total_bytes: 1000,
            used_bytes: 850,
            free_bytes: 150,
            used_pct: 85,
        };
        assert_eq!(compute_bytes_to_free(usage, 70), 150);
    }
}
