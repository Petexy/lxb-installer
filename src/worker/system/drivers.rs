//! Removing the CPU microcode and graphics drivers this computer does not use.
//!
//! What the computer has is read on the live system, which runs on the real
//! hardware: the CPU's vendor from `/proc/cpuinfo`, the graphics devices
//! from `lspci -nn`. The legacy installer's GPU table (`remove_gpu.sh`) is
//! kept row for row, with one change: it never looked for Intel graphics,
//! so a laptop with Intel and NVIDIA graphics lost the Vulkan driver of the
//! screen it draws on. An Intel GPU now always keeps `vulkan-intel`. This
//! runs before the start-up image is built, so a removed driver is not
//! built into it.
use super::{packages, Install};
use crate::model::protocol::{Step, Warning};
use crate::worker::layout::Stop;
use crate::worker::run;

/// NVIDIA's own driver, lib32 first (it depends on the exact nvidia-utils).
pub const NVIDIA_STACK: &[&str] = &["lib32-nvidia-utils", "nvidia-open", "nvidia-utils"];

const NVIDIA: u16 = 0x10de;
const AMD: u16 = 0x1002;
const INTEL: u16 = 0x8086;

/// The first NVIDIA device id of the Turing generation, the oldest the
/// open kernel driver supports.
pub const TURING: u16 = 0x1e00;

/// `Step::Drivers`.
pub fn run(install: &mut Install) -> Result<(), Stop> {
    let mut candidates: Vec<&str> = Vec::new();
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let vendor = cpu_vendor(&cpuinfo);
    run::log(&format!("drivers: CPU vendor {vendor:?}"));
    if let Some(microcode) = vendor.and_then(microcode_to_remove) {
        candidates.push(microcode);
    }
    let mut kept = false;
    match run::run("lspci", &["-nn"]) {
        Ok(listing) => candidates.extend(gpu_packages_to_remove(&listing)),
        Err(stop) => {
            run::log(&format!(
                "drivers: the graphics devices could not be read: {}",
                stop.detail
            ));
            kept = true;
        }
    }
    let total = candidates.len() as u64;
    for (index, name) in candidates.into_iter().enumerate() {
        if packages::installed(install, name) {
            let removed = if name.ends_with("-ucode") {
                packages::remove(install, name)
            } else {
                packages::remove_ignoring_dependents(install, name)
            };
            if let Err(stop) = removed {
                run::log(&format!("drivers: {name} kept: {}", stop.detail));
                kept = true;
            }
        }
        install.progress(Step::Drivers, index as u64 + 1, total);
    }
    if kept {
        install.warn(Warning::DriversKept);
    }
    Ok(())
}

/// The `vendor_id` of the first CPU in `/proc/cpuinfo`.
pub fn cpu_vendor(cpuinfo: &str) -> Option<&str> {
    cpuinfo.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "vendor_id").then(|| value.trim())
    })
}

/// The other vendor's microcode. A CPU from neither (or an unreadable
/// vendor) keeps both: the legacy installer removed Intel's for anything
/// that was not Intel, which is a guess, not a reading.
pub fn microcode_to_remove(vendor: &str) -> Option<&'static str> {
    match vendor {
        "GenuineIntel" => Some("amd-ucode"),
        "AuthenticAMD" => Some("intel-ucode"),
        _ => None,
    }
}

/// The graphics devices `lspci -nn` lists: each display controller's
/// (vendor, device). A display controller is PCI class 03xx (VGA, 3D,
/// other display); the class is the first four-digit code in brackets and
/// the ids are the last `[vvvv:dddd]`, which is how `-nn` prints them.
pub fn graphics_devices(lspci: &str) -> Vec<(u16, u16)> {
    lspci
        .lines()
        .filter_map(|line| {
            let groups: Vec<&str> = line
                .split('[')
                .skip(1)
                .filter_map(|part| part.split(']').next())
                .collect();
            let class = groups
                .iter()
                .find(|g| g.len() == 4 && g.bytes().all(|b| b.is_ascii_hexdigit()))?;
            if !class.starts_with("03") {
                return None;
            }
            let ids = groups.iter().rev().find_map(|g| {
                let (vendor, device) = g.split_once(':')?;
                if vendor.len() != 4 || device.len() != 4 {
                    return None;
                }
                Some((
                    u16::from_str_radix(vendor, 16).ok()?,
                    u16::from_str_radix(device, 16).ok()?,
                ))
            })?;
            Some(ids)
        })
        .collect()
}

/// The graphics packages this computer does not use, in the order the
/// legacy installer removed them. Nothing is removed when no display
/// controller is listed at all: that is an unreadable machine, not one
/// without graphics.
pub fn gpu_packages_to_remove(lspci_nn_text: &str) -> Vec<&'static str> {
    let devices = graphics_devices(lspci_nn_text);
    if devices.is_empty() {
        return Vec::new();
    }
    let nvidia: Vec<u16> = devices
        .iter()
        .filter(|(vendor, _)| *vendor == NVIDIA)
        .map(|(_, device)| *device)
        .collect();
    let amd = devices.iter().any(|(vendor, _)| *vendor == AMD);
    let intel = devices.iter().any(|(vendor, _)| *vendor == INTEL);
    let turing = nvidia.iter().any(|device| *device >= TURING);
    let mut remove: Vec<&'static str> = match (!nvidia.is_empty(), amd, turing) {
        (true, false, true) => vec!["vulkan-radeon", "vulkan-intel", "vulkan-nouveau"],
        (true, false, false) => vec!["vulkan-radeon", "vulkan-intel"],
        (true, true, false) => vec!["vulkan-intel"],
        (true, true, true) => vec!["vulkan-intel", "vulkan-nouveau"],
        (false, true, _) => vec!["vulkan-intel", "vulkan-nouveau"],
        (false, false, _) => vec!["vulkan-radeon", "vulkan-nouveau"],
    };
    if !(turing && !nvidia.is_empty()) {
        remove.extend_from_slice(NVIDIA_STACK);
    }
    if intel {
        remove.retain(|name| *name != "vulkan-intel");
    }
    remove
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTEL_GPU: &str =
        "00:02.0 VGA compatible controller [0300]: Intel Corporation Example Graphics [8086:46a6] (rev 0c)";
    const NVIDIA_NEW: &str =
        "01:00.0 VGA compatible controller [0300]: NVIDIA Corporation Example [GeForce Example] [10de:1e00] (rev a1)";
    const NVIDIA_OLD: &str =
        "01:00.0 VGA compatible controller [0300]: NVIDIA Corporation Example [GeForce Older] [10de:1dff] (rev a1)";
    const NVIDIA_3D: &str =
        "01:00.0 3D controller [0302]: NVIDIA Corporation Example Mobile [10de:2520] (rev a1)";
    const AMD_GPU: &str =
        "03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI] Example [1002:73df] (rev c1)";
    const AMD_AUDIO: &str =
        "03:00.1 Audio device [0403]: Advanced Micro Devices, Inc. [AMD/ATI] Example HDMI Audio [1002:ab28]";
    const VIRTUAL: &str = "00:01.0 VGA compatible controller [0300]: Device [1234:1111] (rev 02)";
    const BRIDGE: &str = "00:00.0 Host bridge [0600]: Intel Corporation Example Host [8086:4621]";

    fn listing(lines: &[&str]) -> String {
        lines.join("\n")
    }

    #[test]
    fn microcode_of_the_other_vendor_goes() {
        assert_eq!(microcode_to_remove("GenuineIntel"), Some("amd-ucode"));
        assert_eq!(microcode_to_remove("AuthenticAMD"), Some("intel-ucode"));
        assert_eq!(microcode_to_remove("HygonGenuine"), None);
        assert_eq!(microcode_to_remove(""), None);
        let cpuinfo = "processor\t: 0\nvendor_id\t: AuthenticAMD\ncpu family\t: 25\n\
                       processor\t: 1\nvendor_id\t: AuthenticAMD\n";
        assert_eq!(cpu_vendor(cpuinfo), Some("AuthenticAMD"));
        assert_eq!(cpu_vendor("processor : 0\n"), None);
    }

    #[test]
    fn only_display_controllers_are_read() {
        let text = listing(&[BRIDGE, INTEL_GPU, AMD_GPU, AMD_AUDIO, NVIDIA_3D]);
        assert_eq!(
            graphics_devices(&text),
            [(0x8086, 0x46a6), (0x1002, 0x73df), (0x10de, 0x2520)]
        );
    }

    #[test]
    fn nvidia_turing_or_newer_alone() {
        assert_eq!(
            gpu_packages_to_remove(NVIDIA_NEW),
            ["vulkan-radeon", "vulkan-intel", "vulkan-nouveau"]
        );
    }

    #[test]
    fn nvidia_older_than_turing_alone() {
        assert_eq!(
            gpu_packages_to_remove(NVIDIA_OLD),
            [
                "vulkan-radeon",
                "vulkan-intel",
                "lib32-nvidia-utils",
                "nvidia-open",
                "nvidia-utils"
            ]
        );
    }

    #[test]
    fn nvidia_older_than_turing_with_amd() {
        assert_eq!(
            gpu_packages_to_remove(&listing(&[AMD_GPU, NVIDIA_OLD])),
            [
                "vulkan-intel",
                "lib32-nvidia-utils",
                "nvidia-open",
                "nvidia-utils"
            ]
        );
    }

    #[test]
    fn nvidia_turing_or_newer_with_amd() {
        assert_eq!(
            gpu_packages_to_remove(&listing(&[AMD_GPU, NVIDIA_NEW])),
            ["vulkan-intel", "vulkan-nouveau"]
        );
    }

    #[test]
    fn amd_alone() {
        assert_eq!(
            gpu_packages_to_remove(&listing(&[AMD_GPU, AMD_AUDIO])),
            [
                "vulkan-intel",
                "vulkan-nouveau",
                "lib32-nvidia-utils",
                "nvidia-open",
                "nvidia-utils"
            ]
        );
    }

    #[test]
    fn neither_nvidia_nor_amd() {
        assert_eq!(
            gpu_packages_to_remove(VIRTUAL),
            [
                "vulkan-radeon",
                "vulkan-nouveau",
                "lib32-nvidia-utils",
                "nvidia-open",
                "nvidia-utils"
            ]
        );
    }

    #[test]
    fn intel_alone_keeps_its_vulkan_driver() {
        let remove = gpu_packages_to_remove(&listing(&[BRIDGE, INTEL_GPU]));
        assert_eq!(
            remove,
            [
                "vulkan-radeon",
                "vulkan-nouveau",
                "lib32-nvidia-utils",
                "nvidia-open",
                "nvidia-utils"
            ]
        );
    }

    #[test]
    fn hybrid_laptops_keep_the_intel_screens_driver() {
        assert_eq!(
            gpu_packages_to_remove(&listing(&[INTEL_GPU, NVIDIA_3D])),
            ["vulkan-radeon", "vulkan-nouveau"]
        );
        assert_eq!(
            gpu_packages_to_remove(&listing(&[INTEL_GPU, NVIDIA_OLD])),
            [
                "vulkan-radeon",
                "lib32-nvidia-utils",
                "nvidia-open",
                "nvidia-utils"
            ]
        );
        assert_eq!(
            gpu_packages_to_remove(&listing(&[INTEL_GPU, AMD_GPU])),
            [
                "vulkan-nouveau",
                "lib32-nvidia-utils",
                "nvidia-open",
                "nvidia-utils"
            ]
        );
    }

    #[test]
    fn any_turing_card_keeps_the_open_driver() {
        assert_eq!(
            gpu_packages_to_remove(&listing(&[NVIDIA_OLD, NVIDIA_NEW])),
            ["vulkan-radeon", "vulkan-intel", "vulkan-nouveau"]
        );
    }

    #[test]
    fn an_empty_listing_removes_nothing() {
        assert!(gpu_packages_to_remove("").is_empty());
        assert!(gpu_packages_to_remove(BRIDGE).is_empty());
    }
}
