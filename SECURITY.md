# Segurança do Nexus

Para relatar uma falha: abra uma *security advisory* privada no GitHub do projeto (não um issue público).

## Superfícies

| Entrada | Quem alcança | Proteção |
|---|---|---|
| `core.sock` (JSON-RPC) e `http.sock` (arte) | só o usuário da sessão | sockets Unix `0600` em `$XDG_RUNTIME_DIR/nexus` |
| HTTP de desenvolvimento (`NEXUS_DEV_HTTP`) | só loopback, desligado por padrão | conferência de `Host` |
| **Remote** `:8787` (controle no celular, envio de ROMs) | a rede local | ver abaixo |
| Apps web (YouTube, Netflix…) | sites externos | navegador separado, sem acesso ao Core |
| Shell (Electron) | só páginas do próprio Nexus | `contextIsolation`, `sandbox`, sem Node, navegação e janelas novas bloqueadas, permissões negadas |

## Remote — modelo de ameaça

**O que protege:** alguém na mesma rede (Wi-Fi da casa, visita, rede de hotel) não pode controlar o Nexus nem gravar
arquivos sem ter visto o QR code na TV.

**Controles** (`core/crates/nexus-core/src/remote.rs` e `net.rs`):

- **Token** de 96 bits aleatórios no QR (`?t=`), comparado em tempo constante. 8 erros por IP → bloqueio de 1 minuto.
- **Validade:** o código vence após **7 dias sem uso** (`NEXUS_REMOTE_TOKEN_TTL`, em segundos); cada conexão válida
  renova o prazo. Vencido, o celular vê "o código expirou" e a tela de pareamento já mostra um novo. "Trocar código"
  invalida na hora e derruba os aparelhos conectados.
- **Só rede local:** pedidos de IPs fora de loopback, faixas privadas, link-local e CGNAT/Tailscale (100.64/10) são
  recusados (403) — um PC ligado direto no modem, com IP público, não fica aberto para a internet.
  `NEXUS_REMOTE_ANY_NET=1` desliga (por sua conta).
- **Origin:** um navegador que manda `Origin` precisa estar na própria página do Remote (mesmo host:porta). Evita
  que um site qualquer aberto no celular ou no PC use um token vazado (CSRF/WebSocket entre sites).
- **Limites:** 16 conexões simultâneas (WebSocket + envios; a 17ª recebe 503), 6 aparelhos pareados, mensagens de
  até 16 KB, 240 mensagens/s por conexão (o excesso é descartado), conexão que não pareia em 20 s é fechada.
  Envio de ROM: até 8 GB, nome higienizado, só extensões de jogos conhecidas, grava em `~/ROMs/<console>/`.
- **Escuta** em `0.0.0.0:8787` (`NEXUS_REMOTE_BIND` muda; `127.0.0.1:8787` desliga o acesso pela rede).

**Riscos aceitos:**

- **Sem TLS.** Na mesma rede, quem consegue capturar tráfego (Wi-Fi aberto, ARP spoofing) vê o token e os comandos.
  Avaliado e adiado: um certificado autoassinado faz o celular mostrar "conexão não segura" a cada pareamento (pior
  que hoje para o usuário comum), e um certificado válido exige domínio público e DNS. Alternativa futura: TLS
  autoassinado com o *fingerprint* no QR, para um app nativo do celular que possa fixar o certificado.
- O token fica na URL: aparece no histórico do navegador do celular. Mitigação: validade e "Trocar código".
- O que o Remote permite é o que um controle permite (navegar, abrir jogos, digitar) e gravar ROMs. Controle dos
  pais (PIN) continua valendo para ações protegidas.

**Testes:** `remote.rs` (`token_validation_and_lockout`, `token_expires_without_use_and_pairing_renews_it`,
`only_local_networks_and_same_origin`, `connection_slots_are_limited_and_released`) e `tests/e2e.rs`
(`remote_pairing_websocket_to_shell_events`, `remote_rejects_wrong_origin_expired_token_and_floods`).

## Sistema

- O instalador só roda como root com `--yes` e mostra o plano antes. Rotas de emergência: `Ctrl+Alt+F3`,
  `/etc/nexus/force-desktop`, `nexus-installer revert-autologin`.
- Pacotes do repositório apt são assinados (ver `docs/APT.md`); a chave privada fica só no segredo do GitHub.
- Apps, emuladores e jogos rodam como o usuário, em escopos systemd próprios; o Core não roda como root.
