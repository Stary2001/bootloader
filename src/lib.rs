/*!
An experimental x86_64 bootloader that works on both BIOS and UEFI systems.

To use this crate, specify it as a dependency in the `Cargo.toml` of your operating system
kernel. Then you can use the [`entry_point`] macro to mark your entry point function. This
gives you access to the [`BootInfo`] struct, which is passed by the bootloader.

## Disk Image Creation

Including the `bootloader` crate as a dependency makes the kernel binary suitable for booting,
but does not create any bootable disk images. To create them, two additional steps are needed:

1. **Locate the source code of the `bootloader` dependency** on your local system. By using the
   dependency source code directly, we ensure that the kernel and bootloader use the same version
   of the [`BootInfo`] struct.
    - When creating a builder binary written in Rust, the
      [`bootloader_locator`](https://docs.rs/bootloader-locator/0.0.4/bootloader_locator/) crate can
      be used to automate this step.
    - Otherwise, the
      [`cargo metadata`](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) subcommand
      can be used to locate the dependency. The command outputs a JSON object with various metadata
      for the current package. To find the `bootloader` source path in it, first look for the
      "bootloader" dependency under `resolve.nodes.deps` to find out its ID (in the `pkg` field).
      Then use that ID to find the bootloader in `packages`. Its `manifest_path` field contains the
      local path to the `Cargo.toml` of the bootloader.
2. **Run the following command** in the source code directory of the `bootloader` dependency to create
   the bootable disk images:

    ```notrust
    cargo builder --kernel-manifest path/to/kernel/Cargo.toml --kernel-binary path/to/kernel_bin
    ```

    The `--kernel-manifest` argument should point to the `Cargo.toml` of your kernel. It is used
    for applying configuration settings. The `--kernel-binary` argument should point to the kernel
    executable that should be used for the bootable disk images.

    In addition to the `--kernel-manifest` and `--kernel-binary` arguments, it is recommended to also
    set the `--target-dir` and `--out-dir` arguments. The former specifies the directory that should
    used for cargo build artifacts and the latter specfies the directory where the resulting disk
    images should be placed. It is recommended to set `--target-dir` to the `target` folder of your
    kernel and `--out-dir` to the the parent folder of `--kernel-binary`.

This will result in the following files, which are placed in the specified `--out-dir`:

- A disk image suitable for BIOS booting, named `boot-bios-<kernel>.img`, where `<kernel>` is the
  name of your kernel executable. This image can be started in QEMU or booted on a real machine
  after burning it to an USB stick..
- A disk image suitable for UEFI booting, named `boot-uefi-<kernel>.img`. Like the BIOS disk image,
  this can be started in QEMU (requires OVMF) and burned to an USB stick to run it on a real
  machine.
- Intermediate UEFI files
  - A FAT partition image named `boot-uefi-<kernel>.fat`, which can be directly started in QEMU
    or written as an EFI system partition to a GPT-formatted disk.
  - An EFI file named `boot-uefi-<kernel>.efi`. This executable is the combination of the
    bootloader and kernel executables. It can be started in QEMU or used to construct a bootable
    disk image: Create an EFI system partition formatted with the FAT filesystem and place the
    EFI file under `efi\boot\bootx64.efi` on that filesystem.

**You can find some examples that implement the above steps [in our GitHub repo](https://github.com/rust-osdev/bootloader/tree/main/examples).**

## Configuration

The bootloader can be configured through a `[package.metadata.bootloader]` table in the
`Cargo.toml` of the kernel (the one passed as `--kernel-manifest`). See the [`Config`] struct
for all possible configuration options.
*/

#![warn(missing_docs)]

use anyhow::Context;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Seek},
    path::Path,
};

mod fat;
mod gpt;
mod mbr;
mod pxe;

const KERNEL_FILE_NAME: &str = "kernel-x86_64";

/// Creates a bootable FAT partition at the given path.
pub fn create_boot_partition(kernel_binary: &Path, out_path: &Path) -> anyhow::Result<()> {
    let bootloader_path = Path::new(env!("UEFI_BOOTLOADER_PATH"));

    let mut files = BTreeMap::new();
    files.insert("efi/boot/bootx64.efi", bootloader_path);
    files.insert(KERNEL_FILE_NAME, kernel_binary);

    fat::create_fat_filesystem(files, &out_path).context("failed to create UEFI FAT filesystem")?;

    Ok(())
}

pub fn create_uefi_disk_image(
    boot_partition_path: &Path,
    out_gpt_path: &Path,
) -> anyhow::Result<()> {
    gpt::create_gpt_disk(boot_partition_path, out_gpt_path)
        .context("failed to create UEFI GPT disk image")?;

    Ok(())
}

pub fn create_bios_disk_image(
    boot_partition_path: &Path,
    out_mbr_path: &Path,
) -> anyhow::Result<()> {
    let bootsector_path = Path::new(env!("BIOS_BOOT_SECTOR_PATH"));
    let second_stage_path = Path::new(env!("BIOS_SECOND_STAGE_PATH"));

    mbr::create_mbr_disk(
        bootsector_path,
        second_stage_path,
        boot_partition_path,
        out_mbr_path,
    )
    .context("failed to create BIOS MBR disk image")?;

    Ok(())
}

/// Prepare a folder for use with booting over UEFI_PXE. The dhcp server should
/// have the filename option set to `bootloader`.
pub fn create_uefi_pxe_tftp_folder(kernel_binary: &Path, out_path: &Path) -> anyhow::Result<()> {
    let bootloader_path = Path::new(env!("UEFI_BOOTLOADER_PATH"));

    pxe::create_uefi_tftp_folder(bootloader_path, kernel_binary, out_path)
        .context("failed to create UEFI PXE tftp folder")?;

    Ok(())
}
