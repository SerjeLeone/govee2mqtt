
check:
	cargo check

test:
	cargo test --lib

fmt:
	cargo +nightly fmt

## ─── Docker (production) ────────────────────────────────────────
docker:
	docker build .

addon:
	docker run \
		--rm \
		--privileged \
		-v /var/run/docker.sock:/var/run/docker.sock \
		-v ./addon:/data \
			ghcr.io/home-assistant/amd64-builder:latest \
			--all \
			--test \
			--target /data

## ─── Local dev stack ────────────────────────────────────────────
## Builds from source + starts Mosquitto + govee2mqtt
## Web UI: http://localhost:8056   MQTT: localhost:1883
dev-up:
	docker compose -f docker-compose.dev.yml up --build -d

dev-down:
	docker compose -f docker-compose.dev.yml down

dev-logs:
	docker compose -f docker-compose.dev.yml logs -f govee2mqtt

dev-rebuild:
	docker compose -f docker-compose.dev.yml up --build -d --force-recreate govee2mqtt

## ─── HA devcontainer ────────────────────────────────────────────
# This will start hass on http://localhost:7123
container:
	npm install @devcontainers/cli
	npx @devcontainers/cli up --workspace-folder .
	npx @devcontainers/cli exec --workspace-folder . supervisor_run

.PHONY: check test fmt docker addon dev-up dev-down dev-logs dev-rebuild container
