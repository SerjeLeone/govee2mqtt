> Fork maintained by [Sergey Leonov (@serjeleone)](https://github.com/serjeleone).
> It is based on [sitapix/govee2mqtt](https://github.com/sitapix/govee2mqtt),
> which derives from the original [wez/govee2mqtt](https://github.com/wez/govee2mqtt) project.
> See [contributors and fork lineage](CONTRIBUTORS.md) for attribution details.

# Govee to MQTT bridge for Home Assistant

This repo provides a `govee` executable whose primary purpose is to act
as a bridge between [Govee](https://govee.com) devices and Home Assistant,
via the [Home Assistant MQTT Integration](https://www.home-assistant.io/integrations/mqtt/).

## Features

* Robust LAN-first design. Not all of Govee's devices support LAN control,
  but for those that do, you'll have the lowest latency and ability to
  control them even when your primary internet connection is offline.
* Support for per-device modes and scenes, including dedicated scene selects
  for lightScene, diyScene, snapshot, nightlightScene, and music modes.
* Categorized scene catalogs enriched with icons and hints, plus Home Assistant
  buttons for cycling to the next or previous scene.
* Support for the undocumented AWS IoT interface to your devices, providing
  low latency status updates.
* Two-factor login support with one-shot verification codes and a configurable
  Govee Home app version for recovering from upstream login changes.
* Support for the official [Platform
  API](https://developer.govee.com/reference/get-you-devices) in case the AWS
  IoT or LAN control is unavailable.
* Real-time state updates via the official Govee MQTT push API (requires API key).
* Per-device and per-segment color control via LAN.
* Device grouping — control multiple devices as one light entity.
* Per-device configuration overrides via JSON file (names, color temp, icons, rooms).
* Web UI with device controls, live log viewer, and bridge status dashboard.
* Native Home Assistant fan entities, air-quality sensors, and diagnostic
  battery/Wi-Fi sensors for supported devices.
* Bounded LAN retries and a per-device polling circuit breaker for congested or
  partially unavailable networks.
* Experimental LAN music-mode palettes for explicitly mapped device models.
* Graceful shutdown with proper MQTT offline status publishing.
* Persistent device database for offline/degraded mode operation.
* Log timestamps use the Home Assistant or host system timezone.

|Feature|Requires|Notes|
|-------|--------|-------------|
|DIY Scenes|API Key|Find in the list of Effects for the light in Home Assistant|
|Music Modes|API Key|Find in the list of Effects for the light in Home Assistant|
|Tap-to-Run / One Click Scene|IoT|Find in the overall list of Scenes in Home Assistant, as well as under the `Govee to MQTT` device|
|Live Device Status Updates|LAN and/or IoT and/or API Key|Devices typically report most changes within a couple of seconds.|
|Segment Color|API Key or LAN|Find the `Segment 00X` light entities associated with your main light device in Home Assistant|
|Energy Monitoring|API Key|Smart plugs expose power, voltage, current, and energy sensors|
|Effect List Filtering|API Key|Disable or filter effects for Google Home compatibility|
|Device Groups|Config file|Control multiple devices as a single HA light entity|
|ptReal Command Replay|LAN or IoT|Send captured DIY scene commands via HTTP API|
|Categorized Scene Catalog|API Key and/or IoT|Scene metadata, current scene, and next/previous controls in Home Assistant|
|Fan Control|API Key and/or IoT|Power, stepped speed, and supported preset modes as an HA fan entity|
|Air Quality and Diagnostics|Device dependent|CO2, PM2.5, PM10, battery, and Wi-Fi entities when reported by the device|
|Custom Music Palettes|LAN + opt-in setting|Experimental multi-colour music mode for explicitly mapped SKUs|

### API Channels

| Channel | Needs | Control | Status | Latency |
|---------|-------|---------|--------|---------|
| LAN | Device on network + LAN enabled | Full (power, color, brightness, scenes, segments) | Real-time broadcast | Lowest |
| IoT | Govee email + password | Full + one-click scenes | Real-time push | Low |
| Platform API | API key | Full except one-click | Poll (120s default) | Medium |
| Govee Push | API key | Read-only | Real-time push | Low |

The bridge automatically picks the best available channel for each device and command.

* `API Key` means that you have [applied for a key from Govee](https://developer.govee.com/reference/apply-you-govee-api-key)
  and have configured it for use in govee2mqtt
* `IoT` means that you have configured your Govee account email and password for
  use in govee2mqtt, which will then attempt to use the
  *undocumented and likely unsupported* AWS MQTT-based IoT service
* `LAN` means that you have enabled the [Govee LAN API](https://app-h5.govee.com/user-manual/wlan-guide)
  on supported devices and that the LAN API protocol is functional on your network

## Usage

* [Installing the HASS App](docs/ADDON.md) - for HAOS and Supervised HASS users
* [Running it in Docker](docs/DOCKER.md)
* [Configuration](docs/CONFIG.md)

## Development

```bash
cp .env.example .env        # fill in your Govee credentials
make dev-up                  # builds from source + starts Mosquitto + govee2mqtt
make dev-logs                # tail logs
make dev-rebuild             # rebuild after code changes
make dev-down                # stop everything
```

Web UI: `http://localhost:8056` | MQTT: `localhost:1883` | Health: `http://localhost:8056/api/health`

### Testing

```bash
make test                              # unit tests
cargo test --test lan_simulator        # LAN protocol simulator
cargo test --test mqtt_integration -- --test-threads=1  # MQTT integration (needs Docker)
```

## MQTT Topics

### Bridge Topics

| Topic | Retained | Description |
|-------|----------|-------------|
| `gv2mqtt/availability` | Yes | Bridge online/offline (LWT) |
| `gv2mqtt/bridge/info` | Yes | Version and state |
| `gv2mqtt/bridge/health` | Yes | Device counts, API status, push stats |
| `gv2mqtt/bridge/devices` | Yes | Full device list with availability |
| `gv2mqtt/bridge/error` | No | Error messages for failed operations |

### Bridge Request/Response API

Publish to these topics to control the bridge via MQTT:

| Request Topic | Payload | Description |
|---------------|---------|-------------|
| `gv2mqtt/bridge/request/health` | (empty) | Publish health data |
| `gv2mqtt/bridge/request/devices` | (empty) | Publish device list |
| `gv2mqtt/bridge/request/cache_purge` | (empty) | Purge caches and re-register |
| `gv2mqtt/bridge/request/config_reload` | (empty) | Reload device config file |
| `gv2mqtt/bridge/request/restart` | (empty) | Restart the bridge |
| `gv2mqtt/bridge/request/log_level` | `trace`/`debug`/`info`/`warn`/`error` | Change log verbosity |

### Per-Device Topics

| Topic | Description |
|-------|-------------|
| `gv2mqtt/{device}/availability` | Per-device online/offline |
| `gv2mqtt/{device}/push_event` | Raw Govee push API events |
| `gv2mqtt/{device}/lack_water` | Humidifier low water alert |
| `gv2mqtt/{device}/scene-catalog` | Retained categorized scene metadata and active scene |
| `gv2mqtt/{device}/scene-next` | Activate the next available scene |
| `gv2mqtt/{device}/scene-prev` | Activate the previous available scene |
| `gv2mqtt/{device}/set-music-palette` | Set an opt-in LAN music-mode palette; see [music mode](docs/MUSIC_MODE.md) |

## HTTP API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/health` | GET | Bridge status (no auth required) |
| `/api/devices` | GET | Device list |
| `/api/device/{id}/inspect` | GET | Full device debug data |
| `/api/device/{id}/power/on` | POST | Turn on |
| `/api/device/{id}/power/off` | POST | Turn off |
| `/api/device/{id}/brightness/{level}` | POST | Set brightness (0-100) |
| `/api/device/{id}/color/{css_color}` | POST | Set color |
| `/api/device/{id}/colortemp/{kelvin}` | POST | Set color temperature |
| `/api/device/{id}/scene/{name}` | POST | Activate scene |
| `/api/device/{id}/scenes` | GET | List available scenes |
| `/api/device/{id}/scene-catalog` | GET | Get categorized scenes, icons, and hints |
| `/api/device/{id}/ptreal` | POST | Send raw ptReal commands |
| `/api/config` | GET/PUT | Read or update device config |
| `/api/oneclicks` | GET | List one-click scenes |
| `/api/oneclick/activate/{scene}` | POST | Activate a one-click scene |
| `/api/logs` | GET | Recent log entries (JSON) |
| `/api/ws/logs` | WebSocket | Live log streaming |

Set `GOVEE_HTTP_AUTH_TOKEN` to require a Bearer token for API access (except `/api/health`).

## Have a question?

* [Is my device supported?](docs/SKUS.md)
* [Check out the FAQ](docs/FAQ.md)

## Credits and attribution

This fork is maintained by [Sergey Leonov (@serjeleone)](https://github.com/serjeleone)
and retains the original authors and contributors in its Git history. See
[CONTRIBUTORS.md](CONTRIBUTORS.md) for the complete fork lineage.

The original project builds on Wez Furlong's earlier work with [Govee LAN
Control](https://github.com/wez/govee-lan-hass/).

The AWS IoT support was made possible by the work of @bwp91 in
[homebridge-govee](https://github.com/bwp91/homebridge-govee/).

The official Govee MQTT push API was discovered via
[govee-java-api](https://github.com/bigboxer23/govee-java-api).

LAN segment color control was contributed by
[alexluckett](https://github.com/alexluckett/govee2mqtt-segment-control).
