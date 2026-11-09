.PHONY: all gen test e2e dev-core build

all: gen build test

gen:           ## regenera os tipos TypeScript a partir do Rust
	pnpm gen

build:
	cargo build --workspace
	pnpm --filter @nexus/shell build
	pnpm --filter @nexus/remote build

test:
	cargo test --workspace
	node --experimental-strip-types packages/focus/test.mjs
	pnpm typecheck

e2e: build  ## cenários de interface contra o Core real (Chromium headless)
	node tests/e2e/run.mjs

dev-core:      ## Core em modo demonstração + Shell em http://127.0.0.1:8799
	cargo run -p nexus-core -- --demo --dev --dev-http 127.0.0.1:8799 --shell-dir apps/shell/dist --remote-dir apps/remote/dist --demo-assets assets/demo --runtime-dir /tmp/nexus-dev/run --data-dir /tmp/nexus-dev/data
