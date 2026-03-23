# Light Scene Parity

This document tracks what `govee2mqtt` already does for Govee light scenes and what still needs work to get closer to app parity.

## Current Coverage

- `lightScene`, `diyScene`, `snapshot`, and `nightlightScene` are exposed as dedicated Home Assistant select entities when the Platform API reports them.
- General scene listing merges multiple sources:
  - Platform API scene list
  - Platform API DIY scene list
  - Undocumented scene catalog fallback
  - Music modes, exposed as `Music: <name>`
- Scene activation uses the Platform API first and falls back to LAN scene packets for supported LAN devices.
- Music mode has dedicated controls for:
  - mode selection
  - sensitivity
  - auto-color
- Effect list can be filtered globally (`GOVEE_DISABLE_EFFECTS`) or per-device (`disable_effects` in device config) for Google Home compatibility.
- Arbitrary captured scene commands can be replayed via `POST /api/device/{id}/ptreal` over LAN or IoT.
- One-click / Tap-to-Run scenes are exposed as HA scene entities and activated via the IoT client.
- Errors during scene activation are published to `gv2mqtt/bridge/error` for automation visibility.

## What "App Parity" Means

For a given light model, parity means:

1. Every app-visible scene name shows up somewhere in `govee2mqtt`.
2. Scenes are grouped into the right buckets:
   - scene
   - DIY scene
   - snapshot
   - night light scene
   - music mode
3. Selecting a scene from Home Assistant triggers the same device behavior as the app.
4. Scene state returns to Home Assistant with the same name the user selected (via the `effect` attribute on the light entity).
5. Missing app-only scenes are identified by source, not guessed at.

## How To Audit A Light

Use the inspect endpoint for a device:

- `GET /api/device/<id>/inspect`

The response includes:

- `platform_scene_names`
- `platform_scene_capability_names` (grouped by instance: lightScene, diyScene, etc.)
- `platform_music_mode_names`
- `undocumented_scene_names`
- `merged_scene_names`

Compare those lists with the scene picker in the Govee app for the same device.

You can also expand a device in the Web UI (click the row) to see capability counts and active scene info.

## Gaps Still Likely

- App ordering, categories, and icons are not represented in Home Assistant.
- Some custom DIY/app-only variants may exist only in app traffic and not in the public Platform API.
- LAN fallback covers known scene-code based scenes for both the legacy `Mode/Scene` select and the dedicated `Scene` select, but not arbitrary captured DIY app payloads (use the ptReal endpoint for those).
- Some devices may expose scene options only through capability shapes we do not currently parse.
- "Sub-scenes" (e.g., "Cha-Cha-B" vs "Cha-Cha-A") are not always exposed — the Platform API may only return the parent scene name.
- Video effects / DreamView sync modes are not available through any API — use Tap-to-Run snapshots as a workaround.
- Game scenes (reported in issue #529) are not exposed through any known API.

## Code Hotspots

- Scene source merging:
  - `src/service/state.rs` — `device_list_scenes`, `device_list_capability_options`
  - `src/platform_api.rs` — `get_merged_enum_capability_by_instance`, `list_scene_names`
  - `src/undoc_api.rs` — `get_scenes_for_device`, `synthesize_platform_api_scene_list`
- Home Assistant entity exposure:
  - `src/hass_mqtt/enumerator.rs` — dedicated scene selects, fallback Mode/Scene select
  - `src/hass_mqtt/select.rs` — `EnumCapabilitySelect`, `SceneModeSelect`, `MusicModeSelect`
  - `src/hass_mqtt/light.rs` — effect list in light entity, effect filtering
- Scene activation:
  - `src/service/state.rs` — `device_set_scene`, `device_set_capability_option`
  - `src/service/hass.rs` — `mqtt_oneclick` (one-click scenes), error feedback
  - `src/hass_mqtt/select.rs` — `mqtt_set_mode_scene`, `mqtt_set_capability_option`
- LAN scene fallback:
  - `src/lan_api.rs` — `set_scene_by_name`, `send_segment_color_rgb`
- ptReal command replay:
  - `src/service/http.rs` — `device_send_ptreal` endpoint
- Debugging and manual comparison:
  - `src/service/http.rs` — inspect endpoint
  - `assets/components/devices.js` — device detail panel with scene info

## Recommended Next Steps

1. Pick one real light SKU and compare app scenes to `/api/device/<id>/inspect`.
2. Record which names are missing and which source should have supplied them.
3. Add fixture coverage for that SKU's scene payloads in `test-data/`.
4. For app-only DIY commands that the Platform API never exposes, use the ptReal replay mechanism to capture and send them.
5. Consider adding sub-scene support by parsing the scene variant suffixes from the undocumented catalog.
