#!/bin/bash
# Uso: tools/compare-engines.sh   (precisa de target/debug/nexus-core, apps/shell/dist, sway, google-chrome ou chromium
# e python3-gi com WebKit2 4.1). Mede RAM (PSS) e CPU de cada motor com a MESMA Shell, 90 s depois de abrir.
# Para a Shell da sessão durante o teste (o Electron só abre uma instância) e religa a sessão no fim.
# Compara motores com a mesma Shell (Core de teste em modo demonstração, sway headless).
set -u
S=$(cd "$(dirname "$0")/engines" && pwd); ROOT=$(cd "$(dirname "$0")/.." && pwd)
export XDG_RUNTIME_DIR=/run/user/$(id -u)
D=$(mktemp -d)
PORT=47811
$ROOT/target/debug/nexus-core --dev --dev-http 127.0.0.1:$PORT --remote-bind 127.0.0.1:47812 --runtime-dir $D/run --data-dir $D/data --shell-dir $ROOT/apps/shell/dist --remote-dir $ROOT/apps/remote/dist --demo-assets $ROOT/assets/demo --demo \
  >$D/core.log 2>&1 &
CORE=$!
env -u WAYLAND_DISPLAY -u DISPLAY WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway -c $S/sway-headless.conf >$D/sway.log 2>&1 &
SWAY=$!
sleep 3
WD=$(ls -t $XDG_RUNTIME_DIR | grep '^wayland-[0-9]*$' | head -1)
systemctl --user stop nexus-shell   # o Electron só abre uma instância por app
echo "sway headless: $WD · Core demo: http://127.0.0.1:$PORT/"
URL=http://127.0.0.1:$PORT/
measure() { # nome cgroup
  local cg=$1 t=0
  for p in $(cat $cg/cgroup.procs); do v=$(awk '/^Pss:/{print $2}' /proc/$p/smaps_rollup 2>/dev/null); t=$((t+${v:-0})); done
  echo $((t/1024)) $(wc -l < $cg/cgroup.procs)
}
run() { # nome comando...
  local name=$1; shift
  local unit=nexus-cmp-$name-$$
  systemd-run --user --scope --quiet --unit=$unit env WAYLAND_DISPLAY=$WD "$@" >$D/$name.log 2>&1 &
  sleep 18
  env WAYLAND_DISPLAY=$WD timeout 5 grim /tmp/nexus-engine-$name.png 2>/dev/null
  sleep 72
  local rel=$(systemctl --user show $unit.scope -p ControlGroup --value)
  [ -n "$rel" ] || { echo "$name: não abriu (ver $D/$name.log)"; tail -3 $D/$name.log; return; }
  local cg=/sys/fs/cgroup$rel
  local u0=$(awk '/^usage_usec/{print $2}' $cg/cpu.stat)
  read m1 n1 < <(measure $cg)
  sleep 30
  local u1=$(awk '/^usage_usec/{print $2}' $cg/cpu.stat)
  read m2 n2 < <(measure $cg)
  printf '%-22s RAM %4s MB  CPU %5s %%  processos %s\n' "$name" "$m2" "$(echo "$u1 $u0" | awk '{printf "%.2f", ($1-$2)/1e4/30}')" "$n2"
  systemctl --user stop $unit.scope 2>/dev/null; sleep 2
}
run electron env NEXUS_RUNTIME_DIR=$D/run NEXUS_KIOSK=1 NEXUS_SHELL_URL=$URL $ROOT/apps/shell/node_modules/electron/dist/electron $ROOT/apps/shell --ozone-platform=wayland --disable-gpu --no-sandbox
run chromium-kiosk google-chrome --ozone-platform=wayland --disable-gpu --no-sandbox --user-data-dir=$D/chrome --no-first-run --no-default-browser-check --disable-sync --kiosk --app=$URL
run webkitgtk-tauri env GDK_BACKEND=wayland WEBKIT_DISABLE_COMPOSITING_MODE=1 python3 -I $S/webkit.py $URL
systemctl --user start nexus-session.target
kill $SWAY $CORE 2>/dev/null; wait 2>/dev/null; rm -rf $D
