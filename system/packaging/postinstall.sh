#!/bin/sh
set -e
udevadm control --reload-rules 2>/dev/null || true
udevadm trigger --subsystem-match=misc --action=add 2>/dev/null || true
modprobe uinput 2>/dev/null || true
if command -v apparmor_parser >/dev/null 2>&1 && [ -f /etc/apparmor.d/opt.nexus.shell.nexus-shell ]; then
  apparmor_parser -r /etc/apparmor.d/opt.nexus.shell.nexus-shell 2>/dev/null || true
fi
systemctl daemon-reload 2>/dev/null || true
