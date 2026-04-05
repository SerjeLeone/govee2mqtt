# Running govee2mqtt in Docker

## Quick Start

1. Ensure that you have configured the MQTT integration in Home Assistant.

    * [follow these steps](https://www.home-assistant.io/integrations/mqtt/#configuration)

2. Copy the example environment file and fill in your values:

```bash
cp .env.example .env
# Edit .env with your Govee credentials and MQTT broker details
```

See [CONFIG.md](CONFIG.md) for a complete list of configuration options.

3. Set up your `docker-compose.yml`:

```yaml
name: govee2mqtt
services:
  govee2mqtt:
    image: ghcr.io/serjeleone/govee2mqtt:latest
    container_name: govee2mqtt
    restart: unless-stopped
    env_file:
      - .env
    # Host networking is required for LAN discovery
    network_mode: host
# Optionally mount the data directory for persistent config and cache:
#    volumes:
#      - '/path/to/data:/data'
```

4. Launch it:

```console
$ docker compose up -d
```

5. Your devices should appear in the MQTT integration in Home Assistant.

6. Access the Web UI at `http://<your-host>:8056`

7. Check health: `http://<your-host>:8056/api/health`

8. Review logs:

```console
$ docker logs govee2mqtt --follow
```

## Per-Device Configuration

Create a `govee-device-config.json` file in your data directory to customize
device names, color temperature ranges, icons, and more. The file is automatically
hot-reloaded — no restart needed. See [CONFIG.md](CONFIG.md) for the full format.

## External Quirks

If a device isn't recognized, you can add it via `govee-quirks.json` in your
data directory. See [CONFIG.md](CONFIG.md) for the format.

## HTTP API Authentication

Set `GOVEE_HTTP_AUTH_TOKEN` in your `.env` to require authentication for API access.
The `/api/health` endpoint is always accessible without a token.

## Development Setup

For building from source and running locally with a test MQTT broker:

```bash
cp .env.example .env
make dev-up        # builds from source + starts Mosquitto + govee2mqtt
make dev-logs      # tail logs
make dev-rebuild   # rebuild after code changes
make dev-down      # stop everything
```

This uses `Dockerfile.dev` (multi-stage source build) and `docker-compose.dev.yml`
(includes Mosquitto broker).
