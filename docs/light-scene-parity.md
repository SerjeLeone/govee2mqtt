# Light Scene Parity

This document tracks what `govee2mqtt` already does for Govee light scenes and what still needs work to get closer to app parity.

## Current Coverage

- `lightScene`, `diyScene`, `snapshot`, and `nightlightScene` can be exposed as Home Assistant select entities when the Platform API reports them.
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
4. Scene state returns to Home Assistant with the same name the user selected.
5. Missing app-only scenes are identified by source, not guessed at.

## How To Audit A Light

Use the inspect endpoint for a device:

- `GET /api/device/<id>/inspect`

The response now includes:

- `platform_scene_names`
- `platform_scene_capability_names`
- `platform_music_mode_names`
- `undocumented_scene_names`
- `merged_scene_names`

Compare those lists with the scene picker in the Govee app for the same device.

## Gaps Still Likely

- App ordering, categories, and icons are not represented in Home Assistant.
- Some custom DIY/app-only variants may exist only in app traffic and not in the public Platform API.
- LAN fallback currently covers known scene-code based scenes, not arbitrary captured DIY app payloads.
- Some devices may expose scene options only through capability shapes we do not currently parse.

## Code Hotspots

- Scene source merging:
  - `src/service/state.rs`
  - `src/platform_api.rs`
  - `src/undoc_api.rs`
- Home Assistant entity exposure:
  - `src/hass_mqtt/enumerator.rs`
  - `src/hass_mqtt/select.rs`
  - `src/hass_mqtt/light.rs`
- LAN scene fallback:
  - `src/lan_api.rs`
- Debugging and manual comparison:
  - `src/service/http.rs`
  - `assets/components/devices.js`

## Recommended Next Steps

1. Pick one real light SKU and compare app scenes to `/api/device/<id>/inspect`.
2. Record which names are missing and which source should have supplied them.
3. Add fixture coverage for that SKU's scene payloads.
4. Only if necessary, add a new import path for app-only DIY commands that the Platform API never exposes.
