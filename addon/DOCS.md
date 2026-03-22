# Govee to MQTT Bridge

This Home Assistant add-on runs `govee2mqtt` inside Home Assistant OS or Supervised Home Assistant and exposes Govee devices to Home Assistant through MQTT discovery.

## What it needs

- A working Home Assistant MQTT integration.
- Usually the Mosquitto Broker add-on, or another reachable MQTT broker.
- Optional Govee credentials if you want cloud, Platform API, or undocumented IoT features.

## Configuration

Common options:

- `temperature_scale`: `C` or `F`
- `govee_email` / `govee_password`: Enables Govee account login features
- `govee_api_key`: Enables official Govee Platform API features
- `mqtt_host` / `mqtt_port` / `mqtt_username` / `mqtt_password`: Override broker auto-discovery if you are not using the Mosquitto add-on
- `debug_level`: Rust log filter such as `govee=trace`
- `no_multicast`, `broadcast_all`, `global_broadcast`, `scan`: LAN discovery tuning

If `mqtt_host` is left empty, the add-on waits for the Home Assistant MQTT service and uses the broker details provided by Supervisor.

## Web UI

The add-on web UI is exposed through Home Assistant Ingress. Open it from the add-on page with **Open Web UI**.

Direct host-network access is intentionally not used for the add-on UI.

## Notes

- `host_network: true` is required because Govee LAN discovery depends on local-network broadcast and multicast traffic.
- Device entities are created by Home Assistant's MQTT integration, not by a Python custom integration in this repository.

## More help

- Repo: https://github.com/wez/govee2mqtt
- User docs: ../docs/ADDON.md
