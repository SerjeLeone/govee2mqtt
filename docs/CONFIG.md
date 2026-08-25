# Configuration Options

## Govee Credentials

While `govee2mqtt` can run without any govee credentials, it can only discover
and control the devices for which you have already enabled LAN control.

It is recommended that you configure at least your Govee username and password
prior to your first run, as that is the only way for `govee2mqtt` to determine
room names to pre-assign your lights into the appropriate Home Assistant areas.

For scene control, for devices that don't support the LAN API, a Govee API Key
is required.  If you don't already have one, [you can find instructions on
obtaining one
here](https://developer.govee.com/reference/apply-you-govee-api-key).

The API key also enables the official Govee MQTT push API for real-time status
updates without polling.

|CLI|ENV|App|Purpose|
|---|---|-----|-------|
|`--govee-email`|`GOVEE_EMAIL`|`govee_email`|The email address you registered with your govee account|
|`--govee-password`|`GOVEE_PASSWORD`|`govee_password`|The password you registered for your govee account|
|`--govee-2fa-code`|`GOVEE_2FA_CODE`|`govee_2fa_code`|One-shot verification code emailed by Govee after a challenged login|
|`--govee-app-version`|`GOVEE_APP_VERSION`|`govee_app_version`|Override the Govee Home app version sent to the undocumented API|
|`--api-key`|`GOVEE_API_KEY`|`govee_api_key`|The API key you requested from Govee support|

### Two-factor login

Leave `govee_2fa_code` empty on the first start. If Govee challenges the
login, the bridge requests a verification email and logs instructions. Enter
the emailed code and restart the bridge within about 15 minutes. The code is
discarded after a successful login so it is not replayed during token refresh.
If the code is rejected or has expired, clear it, restart once without a code
to request a new email, then enter the new code and restart again.

If Govee reports that the app version is too low, set `govee_app_version` to
the current Govee Home version and restart. The bundled fallback is used when
the option is empty.

*Concerned about sharing your credentials? See [Privacy](PRIVACY.md) for
information about how data is used and retained by `govee2mqtt`*

## LAN API Control

A number of Govee's devices support a local control protocol that doesn't require
your primary internet connection to be online.  This offers the lowest latency
for control and is the preferred way for `govee2mqtt` to interact with your
devices.

The [Govee LAN API is described in more detail
here](https://app-h5.govee.com/user-manual/wlan-guide), including a list of
supported devices.

*Note that you must use the Govee Home app to enable the LAN API for each
individual device before it will be possible for `govee2mqtt` to control
it via the LAN API.*

In theory the LAN API is zero-configuration and auto-discovery, but this
relies on your network supporting multicast-UDP, which is challenging
on some networks, especially across wifi access points and routers.

|CLI|ENV|App|Purpose|
|---|---|-----|-------|
|`--no-multicast`|`GOVEE_LAN_NO_MULTICAST=true`|`no_multicast`|Do not multicast discovery packets to the Govee multicast group `239.255.255.250`. It is not recommended to use this option.|
|`--broadcast-all`|`GOVEE_LAN_BROADCAST_ALL=true`|`broadcast_all`|Enumerate all non-loopback network interfaces and send discovery packets to the broadcast address of each one, individually. This may be a good option if multicast-UDP doesn't work well on your network|
|`--global-broadcast`|`GOVEE_LAN_BROADCAST_GLOBAL=true`|`global_broadcast`|Send discovery packets to the global broadcast address `255.255.255.255`. This may be a possible solution if multicast-UDP doesn't work well on your network.|
|`--scan`|`GOVEE_LAN_SCAN=10.0.0.1,10.0.0.2`|`scan`|Specify a list of addresses that should be scanned by sending them discovery packets.|
|N/A|`GOVEE_LAN_LISTEN_PORT=4002`|N/A|Override the LAN response listen port (default 4002). Useful when the Matter Server or another integration conflicts.|

[Read more about LAN API Requirements here](LAN.md)

### Polling behavior on congested networks

Govee2MQTT polls every LAN device for its status at least every 30 seconds
(the pass over all devices is serial, so unresponsive devices stretch the
cycle) and after each command. On a congested 2.4 GHz network, retries for
unresponsive devices used to pile up (up to ~29 packets per device per
status query), making the congestion worse. Two mechanisms bound this; both
are tunable without rebuilding:

|CLI|ENV|App Config|Default|What it does|
|---|---|----------|-------|------------|
|`--lan-query-attempts`|`GOVEE_LAN_QUERY_ATTEMPTS=3`|`lan_query_attempts`|`3`|How many times a status query is sent before giving up. Clamped to 1–100.|
|`--lan-query-backoff-ms`|`GOVEE_LAN_QUERY_BACKOFF_MS=350`|`lan_query_backoff_ms`|`350`|Wait after the first attempt, in milliseconds. Doubles on each retry, capped at 3000 ms (the cap is fixed). Defaults give waits of 350 ms → 700 ms → 1400 ms, ~2.5 s total.|
|`--lan-breaker-threshold`|`GOVEE_LAN_BREAKER_THRESHOLD=3`|`lan_breaker_threshold`|`3`|After this many consecutive timeouts, background polling of that device is suspended (circuit breaker). `0` disables the breaker.|
|`--lan-breaker-cooldown`|`GOVEE_LAN_BREAKER_COOLDOWN=300`|`lan_breaker_cooldown`|`300`|Suspension length in seconds, clamped to 30–900. Doubles on repeated failure, capped at 900 s.|
|`--lan-cloud-fallback <transport>`|`GOVEE_LAN_CLOUD_FALLBACK=platform`|`lan_cloud_fallback`|`disabled`|Optional second transport for segment and `dreamViewToggle` commands: `disabled`, `iot`, or `platform`. LAN is always tried first.|

The tradeoff: lowering attempts makes a congested network recover faster but
makes a slow-to-respond device more likely to report stale state. If you have
a device that reliably answers only after several seconds, raise
`lan_query_attempts` to 4–5. (Above 3 attempts a single query cycle exceeds
the ~5 s post-command confirmation window, so the confirmation poll makes one
full pass instead of re-checking until the commanded value sticks.)

The circuit breaker only affects **background polling**. Commands you send
(turn on, brightness, color) always go out, and their confirmation polls
always run. A suspended device that shows any sign of life — a discovery
response, for example — is granted an immediate status probe instead of
waiting out the cooldown, and the first successful status reply fully
resets the breaker; recovery after an outage therefore takes at most about
a minute. A device that answers discovery but keeps dropping status queries
stays suspended, at the cost of one probe per discovery cycle. Note that a
timed-out confirmation poll still counts toward the breaker threshold even
though it is never blocked: unreachability is evidence no matter which poll
observed it.

### LAN-first feature fallback

Segment color/off and hardware DreamView (`dreamViewToggle`) commands use LAN
`ptReal` first. A `ptReal` UDP write has no acknowledgement, so the bridge runs
one status-query cycle using `lan_query_attempts` and `lan_query_backoff_ms` as a
liveness check. It uses the configured cloud fallback only if those LAN probes
fail or the device was not discovered on LAN. Cloud credentials alone never
enable this fallback.

The `iot` fallback is attempted only when account login has produced an IoT
client and the device metadata contains a usable IoT topic. This explicit
selection overrides conservative automatic-routing defaults in device quirks;
it does not alter transport selection for any other commands.

Scene selection does not use this fallback option. Scenes dynamically prefer a
verified LAN path when the device was discovered locally. Devices without LAN,
and individual scenes that have no local encoding, use the Platform API when
the requested scene is advertised in that device's Platform capabilities.
Decoded IoT scene packets remain the final path when Platform cannot provide
the requested scene.

Independent segment brightness has no verified LAN `ptReal` packet. A
brightness-only segment command therefore needs the `platform` fallback;
segment color and off remain local. Camera-based video sync that is not exposed
as `dreamViewToggle` is not covered by this setting.

See [LAN API requirements and troubleshooting](LAN.md) for port and network
topology details.

## MQTT Configuration

In order to make your devices appear in Home Assistant, you will need to have configured Home Assistant with an MQTT broker.

  * [follow these steps](https://www.home-assistant.io/integrations/mqtt/#configuration)

You will also need to configure `govee2mqtt` to use the same broker:

|CLI|ENV|App|Purpose|
|---|---|-----|-------|
|`--mqtt-host`|`GOVEE_MQTT_HOST`|`mqtt_host`|The host name or IP address of your mqtt broker. This should be the same broker that you have configured in Home Assistant.|
|`--mqtt-port`|`GOVEE_MQTT_PORT`|`mqtt_port`|The port number of the mqtt broker. The default is `1883`|
|`--mqtt-username`|`GOVEE_MQTT_USER`|`mqtt_username`|If your broker requires authentication, the username to use|
|`--mqtt-password`|`GOVEE_MQTT_PASSWORD`|`mqtt_password`|If your broker requires authentication, the password to use|

## Effect List Filtering

If Google Home shows your Govee lights as offline, it's likely because the effect
list exceeds Google's SYNC payload size limit. Use these options to reduce or
disable the published effect list:

|ENV|App|Purpose|
|---|-----|-------|
|`GOVEE_DISABLE_EFFECTS=true`|`disable_effects`|Disable all effects in MQTT discovery. Scene control via automations still works.|
|`GOVEE_ALLOWED_EFFECTS=Forest,Aurora`|`allowed_effects`|Comma-separated whitelist of effects to include (case-insensitive).|

Per-device effect disabling is also available via the [device config file](#per-device-configuration).

## HTTP API Security

|ENV|Purpose|
|---|-------|
|`GOVEE_HTTP_AUTH_TOKEN`|When set, require this token as a Bearer header or `?token=` query param for all API requests. `/api/health` is always accessible without auth.|
|`GOVEE_HTTP_INGRESS_ONLY=true`|Restrict API access to the HA ingress proxy IP only (app use).|

## Per-Device Configuration

Create a JSON file at `govee-device-config.json` in the cache directory (controlled by
`XDG_CACHE_HOME`, or `/data` in the app) to override per-device settings.

The file is **hot-reloaded** — changes are picked up automatically without restart.

```json
{
  "devices": {
    "AA:BB:CC:DD:EE:FF:00:11": {
      "name": "Kitchen Light",
      "color_temp_range": [2700, 6500],
      "room": "Kitchen",
      "disable_effects": true,
      "allowed_effects": ["Forest", "Aurora"],
      "icon": "mdi:ceiling-light"
    },
    "H6076": {
      "prefer_lan": true
    }
  },
  "groups": {
    "all-strips": {
      "name": "All LED Strips",
      "members": ["AA:BB:CC:DD", "EE:FF:00:11"],
      "room": "Living Room"
    }
  }
}
```

### Device Overrides

Keys can be device IDs (exact match) or SKU model numbers (all devices of that model).

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Override device name in HA |
| `color_temp_range` | [min, max] | Override color temperature range in Kelvin |
| `prefer_lan` | bool | Force LAN API when available |
| `disable_effects` | bool | Disable effects for this device |
| `allowed_effects` | [string] | Per-device effect whitelist; takes precedence over the global whitelist |
| `room` | string | Override suggested area in HA |
| `icon` | string | MDI icon override (e.g. `mdi:floor-lamp`) |

### Device Groups

Groups appear as a single light entity in HA. Commands are sent to all members in parallel.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Group name shown in HA |
| `members` | [string] | Device IDs to include |
| `room` | string | Suggested area |
| `icon` | string | MDI icon |

## External Device Quirks

Create a JSON file at `govee-quirks.json` in the cache directory to add or override
device quirks without code changes:

```json
[
  {
    "sku": "H9999",
    "icon": "mdi:lightbulb",
    "supports_rgb": true,
    "supports_brightness": true,
    "color_temp_range": [2700, 6500],
    "lan_api_capable": true,
    "iot_api_supported": true,
    "segment_count": 15,
    "supports_dreamview": true,
    "device_type": "light"
  }
]
```

## Advanced

|ENV|App|Purpose|
|---|-----|-------|
|`RUST_LOG=govee=trace`|`debug_level`|Set log verbosity|
|`GOVEE_LOG_SENSITIVE_DATA=true`|N/A|Include API tokens in logs (debugging only)|
|`GOVEE_CACHE_DIR=/path`|N/A|Override cache directory|
|`GOVEE_TEMPERATURE_SCALE=F`|`temperature_scale`|Use Fahrenheit (default: Celsius)|
|`GOVEE_POLL_INTERVAL=120`|`poll_interval`|Platform API polling interval in seconds (default: 120). Increase to 900 if you have many devices without IoT/LAN support to stay under the 10,000 req/day API limit.|
|`GOVEE_MUSIC_PALETTE=true`|`music_palette`|Enable the experimental LAN-only custom music palette topic; see [Music Mode](MUSIC_MODE.md).|
|`TZ=Europe/Berlin`|N/A|Use this IANA timezone in console, file, and Web UI log timestamps. The Home Assistant app inherits the Supervisor timezone automatically; standalone runs otherwise use the host system timezone.|
