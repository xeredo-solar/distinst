# setup with conftool

/etc/nixos/boot.nix efi


```nix
boot.loader.efi.canTouchEfiVariables = true; # if not --no-efi-vars
boot.loader.efi.efiSysMountPoint = "/boot";
boot.loader.grub.enable = true;
boot.loader.grub.efiSupport = true;
boot.loader.grub.devices = [ "nodev" ];
```

/etc/nixos/boot.nix bios/mbr

```nix
boot.loader.grub.enable = true;
boot.loader.grub.device = "BOOTDEVICE";
```

/etc/nixos/conf-tool.json

```json
{
  "users": ["USERNAME"],
  "keys": {
    "i18n": {
      "timezone": "TIMEZONE",
      "defaultLocale": "LOCALE"
    },
    "console": {
      "keyMap": "KEYBOARD"
    },
    "services": {
      "xserver": {
        "layout": "KEYBOARD"
      }
    }
  }
}
```
+ seed.json

then call conf init --template meros --seed GENERATED_JSON --hwScan --root TARGET & nixos-install --root TARGET -v (possibly use -i option of conf-tool?)

unsure if crypttab is handeled by nixos-generate-config, otherwise need to generate that aswell
