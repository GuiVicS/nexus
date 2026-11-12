#!/bin/sh
# Mede o custo do Nexus no aparelho (rode na sessão Nexus, como o usuário):
#   tools/measure.sh [segundos]     (padrão 30)
# Para cada serviço: RAM real (PSS, soma dos processos), CPU média no intervalo (uso do cgroup) e nº de processos.
# Também o compositor (sway), o jogo/app aberto (se houver) e a RAM usada do sistema inteiro.
set -eu
SEC=${1:-30}
UID_=$(id -u)
BASE=/sys/fs/cgroup/user.slice/user-$UID_.slice/user@$UID_.service
pss() { # soma PSS (kB) de uma lista de pids
  t=0; for p in "$@"; do v=$(awk '/^Pss:/{print $2}' /proc/$p/smaps_rollup 2>/dev/null || echo 0); t=$((t + ${v:-0})); done; echo $((t / 1024))
}
cg_of() { # caminho do cgroup de uma unit de usuário (vazio se não existe)
  c=$(systemctl --user show "$1" -p ControlGroup --value 2>/dev/null || true)
  [ -n "$c" ] && [ -d "/sys/fs/cgroup$c" ] && echo "/sys/fs/cgroup$c" || true
}
usage() { awk '/^usage_usec/{print $2}' "$1/cpu.stat"; }
procs() { cat "$1"/cgroup.procs "$1"/*/cgroup.procs 2>/dev/null | sort -u; }

UNITS="nexus-core nexus-shell nexus-overlay"
GAMECG=$(find "$BASE" -maxdepth 3 -type d -name 'nexus-game*' 2>/dev/null | head -1 || true)
SWAY=$(pgrep -x sway | head -1 || true)
CPUS=$(nproc)
eval_start() { for u in $UNITS; do c=$(cg_of $u); [ -n "$c" ] && eval "S_$(echo $u | tr - _)=$(usage $c)"; done; }
eval_start
SW0=$( [ -n "$SWAY" ] && awk '{print $14+$15}' /proc/$SWAY/stat || echo 0)
G0=$( [ -n "$GAMECG" ] && usage "$GAMECG" || echo 0)
sleep "$SEC"
HZ=$(getconf CLK_TCK)
printf '%-14s %8s %8s %6s\n' "componente" "RAM MB" "CPU %" "procs"
TOT=0
for u in $UNITS; do
  c=$(cg_of $u); [ -n "$c" ] || { printf '%-14s %8s\n' "$u" "parado"; continue; }
  v=S_$(echo $u | tr - _); eval "s=\${$v}"
  e=$(usage $c); ps=$(procs $c); n=$(echo "$ps" | grep -c . || true)
  m=$(pss $ps); TOT=$((TOT + m))
  printf '%-14s %8s %8s %6s\n' "$u" "$m" "$(echo "$e $s $SEC $CPUS" | awk '{printf "%.2f", ($1-$2)/1e4/$3}')" "$n"
done
if [ -n "$SWAY" ]; then
  SW1=$(awk '{print $14+$15}' /proc/$SWAY/stat); m=$(pss $SWAY); TOT=$((TOT + m))
  printf '%-14s %8s %8s %6s\n' "sway" "$m" "$(echo "$SW1 $SW0 $SEC $HZ" | awk '{printf "%.2f", ($1-$2)*100/$4/$3}')" 1
fi
printf '%-14s %8s\n' "Nexus total" "$TOT"
if [ -n "$GAMECG" ]; then
  ps=$(procs "$GAMECG"); printf '%-14s %8s %8s\n' "jogo/app" "$(pss $ps)" "$(echo "$(usage $GAMECG) $G0 $SEC" | awk '{printf "%.2f", ($1-$2)/1e4/$3}')"
fi
echo "sistema: $(free -m | awk '/^(Mem|Mem.):/{print $3}') MB usados de $(free -m | awk '/^(Mem|Mem.):/{print $2}') · CPU % = de 1 núcleo (100 = um núcleo inteiro; $CPUS núcleos)"
