extern crate json;
use self::json::object;
use self::json::array;
use self::json::JsonValue;
use self::json::parse;

use NO_EFI_VARIABLES;
use crate::installer::{conf::RecoveryEnv};
use errors::*;
use disks::{Bootloader, Disks};
use installer::traits::InstallerDiskOps;
use std::{
    path::Path,
    path::Component,
    process::{Command, Stdio},
    fs,
    io::{self, BufReader, BufRead},
    sync::atomic::Ordering,
    os::unix::fs::PermissionsExt,
};
use timezones::Region;
use Config;
use UserAccountCreate;

const USE_STATUS: [(&str, f64); 3] = [
    /* status, weight*/
    ("builds", 1.0),
    ("copyPath", 0.1),
    ("download", 0.3)
];

#[macro_export]
macro_rules! str {
    ($var:expr) => {
        JsonValue::String($var);
    }
}


pub fn nixos<P: AsRef<Path>, F: FnMut(i32)>(
    recovery_conf: Option<&mut RecoveryEnv>,
    bootloader: Bootloader,
    disks: &Disks,
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

    let boot = generate_boot_config(
        disks,
        bootloader
    );

    let nix_conf_folder = mount_dir.join("etc/nixos");

    let target = mount_dir.to_str().unwrap();

    fs::create_dir_all(&nix_conf_folder).expect("failed to mkdir /etc/nixos on target");
    fs::write(nix_conf_folder.join("conf-tool.json"), json).expect("failed to write /etc/nixos/conf-tool.json");
    fs::write(nix_conf_folder.join("boot.nix"), boot).expect("failed to write /etc/nixos/boot.nix");
    // nixos-install: path $PARENT should have permissions 755, but had permissions 750. Consider running $CMD
    // tmp make parent have permission 755
    let target_parent = mount_dir.parent().unwrap();
    let target_parent_perm = fs::metadata(target_parent)?.permissions();
    fs::set_permissions(target_parent, fs::Permissions::from_mode(0o755)).expect("failed to update permissions for install-parent to 755");

    info!("setting up");

    let init = Command::new("conf")
            .arg("init")
            .arg("--root")
            .arg(target)
            .arg("--template")
            .arg("solaros")
            .arg("--hwScan")
            .status()
            .expect("failed to execute init command");

    if !init.success() {
        io::Error::new(io::ErrorKind::Other, "failed to init");
    }

    info!("running nixos-install");

    let mut install = Command::new("nixos-install-wrapped")
            .arg("--root")
            .arg(target)
            .arg("-v")
            .arg("--show-trace")
            .env("LOGFILE", mount_dir.join("install.log").to_str().unwrap())
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute install command");

    if let Some(ref mut stderr) = install.stderr {
        for line in BufReader::new(stderr).lines() {
            match progress(line.unwrap()) {
                Some(p) => {
                    callback((p * 100.0) as i32);
                },
                _ => ()
            }
        }
    }

    fs::set_permissions(target_parent, target_parent_perm).expect("failed to update permissions for install-parent to previous value");

    let exit_status = install.wait().expect("failed to install");

    if !exit_status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "failed to install"));
    }

    Ok(())
}

fn generate_conftool_json<D: InstallerDiskOps>(
    recovery_conf: Option<&mut RecoveryEnv>,
    disks: &D,
    config: &Config,
    region: Option<&Region>,
    user: Option<&UserAccountCreate>,
    extra_config: Option<JsonValue>
) -> String {
    let mut j = if extra_config.is_some() { extra_config.clone().unwrap() } else { object!{} };

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
    j["keys"]["networking"]["hostName"] = str!(config.hostname.clone());
    j["keys"]["services"]["xserver"]["layout"] = str!(config.keyboard_layout.clone());

    if config.keyboard_model.is_some() {
        j["keys"]["services"]["xserver"]["xkbModel"] = str!(config.keyboard_model.clone().unwrap());
    }

    if config.keyboard_variant.is_some() {
        j["keys"]["services"]["xserver"]["xkbVariant"] = str!(config.keyboard_variant.clone().unwrap());
    }

    // TODO: crypttab? flags?

    return json::stringify(j);
}

macro_rules! ap {
    ($target:expr, [ $($str:expr),+ ]) => {
        $($target += $str;)+
    }
}

macro_rules! ap_nix {
    ($target:expr, $key:expr, $val:expr) => {
        ap!($target, [ "  ", $key, " = ", $val, ";\n" ]);
    }
}

macro_rules! quote {
    ($str:expr) => {
        &format!("\"{}\"", $str)
    }
}

fn generate_boot_config(
    disks: &Disks,
    bootloader: Bootloader,
) -> String {
    let mut conf: String = "{ config, pkgs, lib, ... }:".to_string();

    let ((root_dev, _root_part), boot_opt) = disks.get_base_partitions(bootloader);

    let mut efi_part_num = 0;

    let bootloader_dev = boot_opt.map_or(root_dev, |(dev, dev_part)| {
        efi_part_num = dev_part.number;
        dev
    });

    ap!(conf, [
        "# Boot settings, be careful\n\n",
        "{"
    ]);

    match bootloader {
        Bootloader::Bios => {
            ap_nix!(conf, "boot.loader.grub.enable", "true");
            ap_nix!(conf, "boot.loader.grub.device", quote!(bootloader_dev.to_str().unwrap().to_owned()));
        }
        Bootloader::Efi => {
            ap_nix!(conf, "boot.loader.canTouchEfiVariables", if NO_EFI_VARIABLES.load(Ordering::Relaxed) { "false" } else { "true" }); // if not --no-efi-vars
            ap_nix!(conf, "boot.loader.efi.efiSysMountPoint", quote!("/boot/efi")); // maybe get from disk ops?
            ap_nix!(conf, "boot.loader.grub.enable", "true");
            ap_nix!(conf, "boot.loader.grub.efiSupport", "true");
            ap_nix!(conf, "boot.loader.grub.devices", "[ \"nodev\" ]");
        }
    }

    conf += "}\n";

    return conf
}

fn progress(data: String) -> Option<f64> {
    struct Status {
        done: u64,
        expected: u64,
        weight: f64,
    }

    fn to_status(data: &JsonValue) -> Result<Vec<Status>, io::Error> {
        let mut status: Vec<Status> = Vec::new();
        for &(name, weight) in USE_STATUS.iter() {
            let item = &data[name];
            status.push(Status {
                done: match item["done"].as_u64() {
                    Some(x) => x,
                    _ => return Err(io::Error::new(
                            io::ErrorKind::InvalidInput, "Cannot find name"))
                },
                expected: match item["expected"].as_u64() {
                    Some(x) => x,
                    _ => return Err(io::Error::new(
                            io::ErrorKind::InvalidInput, "Cannot find name"))
                },
                weight: weight,
            });
        }
        Ok(status)
    }

    fn percentage(data: Vec<Status>) -> Option<f64> {
        let mut done = 0f64;
        let mut expected = 0f64;

        for i in data {
            done += i.done as f64 * i.weight;
            expected += i.expected as f64 * i.weight;
        }

        if expected == 0.0 || done == expected {
            None
        } else {
            Some(done / expected)
        }
    }

    let parsed = parse(&data).unwrap();
    let status = to_status(&parsed["status"]).unwrap();
    percentage(status)
}
