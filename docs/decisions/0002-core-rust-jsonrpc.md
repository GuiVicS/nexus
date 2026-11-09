# ADR 0002 — Core em Rust, JSON-RPC em socket Unix
Socket `$XDG_RUNTIME_DIR/nexus/core.sock` (0600), uma mensagem JSON por linha. A Shell só envia IDs e intenções;
permissões (perfil infantil, PIN, limite de tempo) são decididas no Core. HTTP na rede local existe apenas para o
Remote (token de 96 bits + bloqueio por IP). Tipos TypeScript são gerados com ts-rs.
