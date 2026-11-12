# Finalização do Nexus — orquestração por subagentes

Pedido do usuário (2026-11-12): terminar tudo rápido, com subagentes Claude, um responsável por fase, o modelo
escolhido pelo tamanho da tarefa (economia de tokens).

## Hierarquia

```
Orquestrador (Opus 5.5, sessão principal)
│  plano, escopo de cada agente, integração (merge na main), suíte completa, decisões, release
│
├── Onda 1 (em paralelo, cada um na sua cópia do repositório / git worktree)
│   ├── A · Fase 2 do sistema ........ Sonnet  A1 barra de status, A2 Central rápida, A3 overlay no jogo,
│   │                                           A4 notificações, B10 energia, B9 armazenamento (disco/USB)
│   ├── B · Fase 3 do sistema + SSH ... Sonnet  B6 controles, B7 celular, B11 atualizações, B15 recuperação
│   │                                           (inclui SSH liga/desliga), B16 sobre/diagnóstico + passo a passo
│   │                                           de acesso por SSH pelo terminal em Ajustes
│   ├── C · Mídia (Drive parte 2) ..... Sonnet  aba Mídia com filmes/séries/músicas da pasta Nexus do Drive,
│   │                                           player mpv em tela cheia, mpv nas dependências
│   └── D · Logos (refino) ............ Haiku   baixar as 22 logomarcas que faltam (Commons limitou), créditos,
│                                               lista SYS_LOGO, conferir cada card em captura
│
├── Integração 1 (Orquestrador): merge A→B→C→D, conflitos, suíte completa
│
├── Onda 2
│   ├── E · Fase 4 .................... Sonnet  A5 energia única, B5 CEC, B12 perfis/parental, B13 aparência,
│   │                                           B14 acessibilidade, B17 apps instalados (+ .jar/AppImage/.sh
│   │                                           sem atalho, ex.: TLauncher), B18 ROMs, B19 privacidade
│   └── F · Textos (refino) ........... Haiku   revisar português de todas as telas novas, README/HANDOFF/STATUS
│                                               consolidados (sem mudar código de lógica)
│
└── Integração 2 + encerramento (Orquestrador)
    suíte completa, memória da Shell, versão 0.5.0 (.deb/pacotes locais); push e release só com o OK do usuário;
    encerramento da §4b do HANDOFF (zerar a VM e mostrar o assistente) quando o usuário pedir
```

**Por que esses modelos:** Opus só onde há decisão e integração (conflitos, arquitetura, critério de pronto). Sonnet nas
fases: código Rust + Solid + testes, muitas telas, bom custo. Haiku em refino mecânico (baixar arquivos, listas,
créditos, revisar texto), onde não há lógica nova.

**Fora do alcance dos agentes (precisa do usuário):** teste num PC de verdade; login real no Google; `sudo` na VM;
chave GPG/Pages do apt; publicar release; o modo "encerrar a Shell" (decisão pendente).

## Regras para todo agente

1. Leia `HANDOFF.md` (§4b tem o detalhe de cada item) e `CLAUDE.md`. Responda e escreva textos em pt-BR.
2. Trabalhe só no seu escopo. Arquivos compartilhados (`rpc.rs`, `api.ts`, `scenarios.mjs`, `Settings.tsx`,
   `actions.ts`, `styles.css`, `docs/STATUS.md`): só **acrescente**, não reorganize nem reformate o que é dos outros.
3. Mesmos padrões do código existente (ver HANDOFF §4b "Entregáveis"): Core faz tudo do sistema (Shell nunca roda nada
   privilegiado); argv fixo, entrada validada; programa externo com variável `NEXUS_<PROG>` para o teste trocar por um
   falso em `tests/e2e/fake/`; estados carregando/vazio/erro; ação perigosa = PIN + confirmação; só controle.
4. **Disco quase cheio (3,5 GB) e 5 núcleos para todos:**
   - `export CARGO_TARGET_DIR=$HOME/nexus/target CARGO_INCREMENTAL=0` (target compartilhado; não crie outro);
   - compile e copie o binário sob trava, e teste a UI com ele:
     `flock /tmp/claude-1000/nexus-build.lock sh -c 'cargo build -q -p nexus-core && cp $CARGO_TARGET_DIR/debug/nexus-core .nexus-core-bin'`
     e `NEXUS_CORE_BIN=$PWD/.nexus-core-bin NEXUS_EMULATION=0 node tests/e2e/run.mjs <cenário>`;
   - `pnpm install --offline --frozen-lockfile` uma vez na sua cópia; depois `pnpm -C apps/shell build`;
   - `.nexus-core-bin` não vai para o git.
5. Testes: `cargo test -q -p nexus-core` (com o `CARGO_TARGET_DIR` acima), `pnpm -C apps/shell exec tsc --noEmit -p .`,
   o seu cenário de UI novo e os cenários vizinhos que você mexeu. Os de UI são sensíveis a carga: se falhar, repita
   antes de supor bug. Capturas 1080p/720p com `ui.screens('<item>')` e `NEXUS_SHOTS=<sua pasta temporária>`; olhe-as.
6. Proibido: `sudo`; push/release; reiniciar `nexus-core`/`nexus-shell` do systemd do usuário ou mexer na sessão sway
   aninhada que está rodando; `pkill -f` amplo; responder EULA/licença ou digitar credenciais; mudar `~/.config`.
7. Ao terminar: entrada no fim de `docs/STATUS.md` (o que foi feito, testes, capturas, o que ficou faltando e por quê),
   **commit na sua branch** (mensagem em pt-BR + linha `Co-Authored-By: Claude …`), e responda ao orquestrador com:
   branch, commits, testes rodados (com resultado), o que não foi feito.

## Como terminou (2026-11-12) e lições para a próxima orquestração

Resultado: os seis agentes entregaram; tudo entrou na `main` e saiu na **release v0.5.0** (35 cenários de UI e todos os testes do Core passando).
O merge foi feito na ordem A (Fase 2) → B (Fase 3) → C (Mídia) → E (Fase 4) → F (textos) → D (logos), conforme cada um terminava.

Lições:
- **Cópia de trabalho (worktree) nasce da `origin/main`.** Faça o push antes de lançar os agentes, senão eles partem de uma base velha
  (o agente D chegou a ver a `main` "sem compilar" por isso).
- **Limite de uso da conta:** os agentes param no meio com erro 429. O trabalho fica salvo; quando o limite volta, mande uma mensagem
  ao mesmo agente (SendMessage) para ele continuar. Não crie outro.
- **Conflitos previsíveis:** `scenarios.mjs` e `lib.mjs` (todos acrescentam no fim), `styles.css`, `STATUS.md`, `lib.rs`, `Settings.tsx`,
  `build-deb.sh` (`Depends`). Na maioria, manter os dois lados resolve. No `scenarios.mjs`, monte a versão da `main` + a cauda nova da
  branch, nunca a união cega dos blocos (a união quebrou a sintaxe), e procure nomes de cenário repetidos.
- **Agente pequeno (Haiku) em tarefa de rede lenta** (Commons com 429) fica esperando muito tempo. Mande fechar com o que já tem.
- **Confira o visual você mesmo:** o agente de logos só olhou a primeira página da grade, e sete logos viravam mancha branca.
- **Rode a suíte completa depois de cada merge:** os cenários dos agentes passaram isolados, mas a mudança de versão quebrou um teste
  (`atualizacoes`) que nenhum agente viu.
- Depois do merge, remova a worktree. Se estiver travada pelo processo do agente: `git worktree unlock` e depois `git worktree remove --force`.
