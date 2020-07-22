#!/bin/bash

if [ -z "$1" ]; then
  echo "Needs device..." >&2
  exit 2
fi

export RUST_BACKTRACE=1

exec sudo -E target/*/distinst \
    -h "solaros-testing" \
    -k "us" \
    -l "en_US.UTF-8" \
    -b "$1" \
    -t "$1:gpt" \
    -n "$1:primary:start:512M:fat32:mount=/boot/efi:flags=esp" \
    -n "$1:primary:512M:-4096M:ext4:mount=/" \
    -n "$1:primary:-4096M:end:swap" \
    --username "solaros" \
    --realname "solarOS User" \
    --tz "Etc/UTC"
