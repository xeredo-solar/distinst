#!/bin/sh

if [ $(id -ru) != "0" ]; then
  echo "ERROR: Needs root" >&2
fi

set -euxo pipefail

OUT=$(readlink -f "$PWD/../disk.img")
SIZE=$(( 1024 * 1024 * 1024 * 16 ))

get_ldev() {
  losetup | grep "$OUT" | sed -r "s|^(/dev[^ ]+).+$|\1|g"
}

for dev in $(get_ldev); do
  losetup -d "$dev"
done

dd if=/dev/zero "of=$OUT" bs=1 count=0 "seek=$SIZE"
losetup -Pf "$OUT"

get_ldev
