#!/bin/sh
# Modo simulação: o Nexus como ele funciona no aparelho, mas numa janela de navegador.
#   - sem catálogo fictício: começa no primeiro uso (crie seu perfil);
#   - Steam simulada (tools/sim/steam) com conta, 4 jogos instalados e 3 já jogados para instalar;
#     baixar mostra progresso real lido dos arquivos da Steam; jogar abre um "jogo" que o Nexus acompanha e fecha;
#   - Loja: a vitrine pública de verdade da Steam (precisa de internet);
#   - Apps e jogos: os programas instalados nesta máquina (abrem de verdade);
#   - os dados ficam em ~/.local/share/nexus-sim (persistem entre execuções).
# Uso: tools/sim.sh [--reset]   →  abra http://127.0.0.1:8799
set -eu
cd "$(dirname "$0")/.."
REPO=$(pwd)
SIM="${NEXUS_SIM_DIR:-$HOME/.local/share/nexus-sim}"
PORT="${NEXUS_SIM_PORT:-8799}"

if [ "${1:-}" = "--reset" ]; then
  rm -rf "$SIM"
  echo "Simulação apagada ($SIM)."
fi

ROOT="$SIM/Steam"
if [ ! -d "$ROOT" ]; then
  echo "Preparando a Steam simulada em $ROOT…"
  mkdir -p "$ROOT/steamapps" "$ROOT/config" "$ROOT/userdata/100000001/config"
  cat > "$ROOT/config/loginusers.vdf" <<'EOF'
"users"
{
	"76561198060265729"
	{
		"AccountName"		"jogador"
		"PersonaName"		"Jogador Simulado"
		"MostRecent"		"1"
	}
}
EOF
  installed() { # appid nome tamanho
    printf '"AppState"\n{\n\t"appid"\t\t"%s"\n\t"name"\t\t"%s"\n\t"StateFlags"\t\t"4"\n\t"installdir"\t\t"%s"\n\t"SizeOnDisk"\t\t"%s"\n}\n' "$1" "$2" "$2" "$3" > "$ROOT/steamapps/appmanifest_$1.acf"
  }
  installed 570 "Dota 2" 38000000000
  installed 730 "Counter-Strike 2" 33000000000
  installed 440 "Team Fortress 2" 26000000000
  installed 620 "Portal 2" 12000000000
  NOW=$(date +%s)
  # jogos já abertos nesta conta (sem chave da Web API é assim que a Steam real informa "jogos da conta")
  cat > "$ROOT/userdata/100000001/config/localconfig.vdf" <<EOF
"UserLocalConfigStore"
{
	"Software"
	{
		"Valve"
		{
			"Steam"
			{
				"apps"
				{
					"570"		{ "Playtime"		"5400"	"LastPlayed"		"$((NOW - 3600))" }
					"620"		{ "Playtime"		"610"	"LastPlayed"		"$((NOW - 86400 * 3))" }
					"413150"	{ "Playtime"		"2900"	"LastPlayed"		"$((NOW - 86400 * 20))" }
					"105600"	{ "Playtime"		"1200"	"LastPlayed"		"$((NOW - 86400 * 45))" }
					"1145360"	{ "Playtime"		"880"	"LastPlayed"		"$((NOW - 86400 * 60))" }
				}
			}
		}
	}
}
EOF
fi

[ -x target/debug/nexus-core ] && [ -f apps/shell/dist/index.html ] && [ -f apps/remote/dist/index.html ] || {
  echo "Compilando (make build)…"
  make build
}

echo "Nexus (simulação) em http://127.0.0.1:$PORT  ·  Ctrl+C para parar"
echo "Log da Steam simulada: $SIM/steam.log"
export NEXUS_SIM_DIR="$SIM"
export NEXUS_STEAM_ROOT="$ROOT"
export NEXUS_STEAM_BIN="$REPO/tools/sim/steam"
export NEXUS_DATA_DIR="$SIM/data"
export NEXUS_CACHE_DIR="$SIM/cache"
export NEXUS_UINPUT=0
exec "$REPO/target/debug/nexus-core" --dev --dev-http "127.0.0.1:$PORT" \
  --shell-dir apps/shell/dist --remote-dir apps/remote/dist \
  --runtime-dir "$SIM/run" --data-dir "$SIM/data"
