extern crate json;
use self::json::object;
use self::json::array;
use self::json::JsonValue;

use bootloader::Bootloader;
use super::{mount_cdrom, mount_efivars};
use crate::installer::{conf::RecoveryEnv, steps::normalize_os_release_name};
use chroot::Chroot;
use distribution;
use errors::*;
use external::remount_rw;
use hardware_support;
use installer::traits::InstallerDiskOps;
use libc;
use misc;
use os_release::OsRelease;
use partition_identity::PartitionID;
use rayon;
use std::{
    fs::{self, Permissions},
    io::{self, Write},
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::Path,
    path::Component,
    process::Command
};
use tempdir::TempDir;
use timezones::Region;
use Config;
use UserAccountCreate;
use INSTALL_HARDWARE_SUPPORT;

#[macro_export]
macro_rules! str {
    ($var:expr) => {
        JsonValue::String($var);
    }
}

pub fn nixos<D: InstallerDiskOps, P: AsRef<Path>, F: FnMut(i32)>(
    recovery_conf: Option<&mut RecoveryEnv>,
    disks: &D,
    mount_dir: P,
    config: &Config,
    region: Option<&Region>,
    user: Option<&UserAccountCreate>,
    mut callback: F,
) -> io::Result<()> {
    let mount_dir = mount_dir.as_ref().canonicalize().unwrap();

    info!("writing config");

    let seed = Path::new("/etc/conf-tool-seed.json");
    let mut extra_config: Option<JsonValue> = None;
    if seed.exists() {
        extra_config = Some(
            json::parse(
                &fs::read_to_string(seed).expect("failed to read conf-tool seed")
            ).expect("failed to parse conf-tool seed")
        );
    }

    let json = generate_conftool_json(
        recovery_conf,
        disks,
        config,
        region,
        user,
        extra_config
    );

    let nix_conf_folder = mount_dir.join("etc/nixos");

    let target = mount_dir.to_str().unwrap();

    fs::create_dir_all(&nix_conf_folder).expect("failed to mkdir /etc/nixos on target");
    fs::write(nix_conf_folder.join("conf-tool.json"), json).expect("failed to write /etc/nixos/conf-tool.json");

    info!("setting up config");

    let init = Command::new("conf")
            .arg("init")
            .arg("--root")
            .arg(target)
            .arg("--template")
            .arg("meros")
            .arg("--hwScan")
            .status()
            .expect("failed to execute init command");

    if !init.success() {
        io::Error::new(io::ErrorKind::Other, "failed to init");
    }

    info!("running nixos-install");

    let install = Command::new("nixos-install")
            .arg("--root")
            .arg(target)
            .arg("-L")
            .arg("-v")
            .output()
            .expect("failed to execute install command");

    // TODO: somehow update status while installing

    if !install.status.success() {
        io::Error::new(io::ErrorKind::Other, "failed to install");
    }

    Ok(())
}

fn generate_conftool_json<D: InstallerDiskOps>(
    recovery_conf: Option<&mut RecoveryEnv>,
    disks: &D,
    config: &Config,
    region: Option<&Region>,
    user: Option<&UserAccountCreate>,
    extraConfig: Option<JsonValue>
) -> String {
    let mut j = if extraConfig.is_some() { extraConfig.clone().unwrap() } else { object!{} };

    if user.is_some() {
        let u = user.unwrap();
        j["keys"]["users"] = array![u.username.clone()];

        if u.password.is_some() {
            j["keys"]["users"]["users"][u.username.clone()]["initialPassword"] = str!(u.password.clone().unwrap());
        }

        if u.realname.is_some() {
            j["keys"]["users"]["users"][u.username.clone()]["name"] = str!(u.realname.clone().unwrap());
        }
    }

    if region.is_some() && region.unwrap().path().to_str().is_some() {
        let mut comp = region.unwrap().path().components();
        while comp.next().unwrap() != Component::Normal("zoneinfo".as_ref()) {
            // do nothing
        }
        let tz = comp.as_path().to_str().unwrap().to_string();
        j["keys"]["time"]["timeZone"] = str!(tz);
    }

    j["keys"]["i18n"]["defaultLocale"] = str!(config.lang.clone());
    j["keys"]["console"]["useXkbConfig"] = JsonValue::Boolean(true);
    j["keys"]["networking"]["hostname"] = str!(config.hostname.clone());
    j["keys"]["services"]["xserver"]["layout"] = str!(config.keyboard_layout.clone());

    if config.keyboard_model.is_some() {
        j["keys"]["services"]["xserver"]["xkbModel"] = str!(config.keyboard_model.clone().unwrap());
    }

    if config.keyboard_variant.is_some() {
        j["keys"]["services"]["xserver"]["xkbVariant"] = str!(config.keyboard_variant.clone().unwrap());
    }

    // TODO: crypttab? flags? what is region?

    return json::stringify(j);
}
