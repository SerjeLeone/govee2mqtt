use crate::ble::NotifyHumidifierNightlightParams;
use crate::commands::serve::POLL_INTERVAL;
use crate::lan_api::{DeviceColor, DeviceStatus as LanDeviceStatus, LanDevice};
use crate::platform_api::{
    DeviceCapability, DeviceCapabilityState, DeviceType, HttpDeviceInfo, HttpDeviceState,
};
use crate::service::quirks::{resolve_quirk, Quirk, BULB};
use crate::service::state::SceneCatalogCache;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::IpAddr;

#[derive(Default, Clone, Debug)]
pub struct Device {
    pub sku: String,
    pub id: String,

    /// Probed LAN device information, found either via discovery
    /// or explicit probing by IP address
    pub lan_device: Option<LanDevice>,
    pub last_lan_device_update: Option<DateTime<Utc>>,

    pub lan_device_status: Option<LanDeviceStatus>,
    pub last_lan_device_status_update: Option<DateTime<Utc>>,

    pub http_device_info: Option<HttpDeviceInfo>,
    pub last_http_device_update: Option<DateTime<Utc>>,

    pub http_device_state: Option<HttpDeviceState>,
    pub last_http_device_state_update: Option<DateTime<Utc>>,

    pub undoc_device_info: Option<UndocDeviceInfo>,
    pub last_undoc_device_info_update: Option<DateTime<Utc>>,

    pub iot_device_status: Option<LanDeviceStatus>,
    pub last_iot_device_status_update: Option<DateTime<Utc>>,

    /// Latest explicit observation of this device's connectivity through a
    /// Govee cloud transport. Kept separately from `DeviceState`: a newer LAN
    /// state must not erase a valid cloud status in the Web UI.
    cloud_online: Option<bool>,
    last_cloud_status_update: Option<DateTime<Utc>>,
    last_cloud_probe_attempt: Option<DateTime<Utc>>,

    /// When an IoT packet last carried an explicit `state.mode`.
    /// `last_iot_device_status_update` is re-stamped by every IoT packet,
    /// including mode-less ones whose merge carries the cached mode
    /// forward, so it cannot serve as the mode observation time.
    /// Stamped by the IoT subscriber merge (see service/iot.rs).
    pub last_iot_mode_update: Option<DateTime<Utc>>,

    pub nightlight_state: Option<NotifyHumidifierNightlightParams>,
    pub target_humidity_percent: Option<u8>,
    pub humidifier_work_mode: Option<u8>,
    pub humidifier_param_by_mode: HashMap<u8, u8>,

    pub last_polled: Option<DateTime<Utc>>,

    /// Set when the Platform API returns "devices not belong you".
    /// Skips polling until cooldown expires, then retries in case
    /// the device was re-added to the account. Resets on restart.
    pub(crate) platform_not_belong_until: Option<DateTime<Utc>>,

    active_scene: Option<ActiveSceneInfo>,
    active_music_mode: Option<ActiveMusicModeInfo>,
    /// DreamView is not reported by LAN devStatus, so local/IoT commands are
    /// tracked optimistically until a Platform state poll supplies a value.
    dreamview_enabled: Option<bool>,
    /// Cached merged scene catalog, including category and image metadata.
    scene_catalog_cache: Option<SceneCatalogCache>,
}

impl std::fmt::Display for Device {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{} ({} {})", self.name(), self.id, self.sku)
    }
}

/// Govee doesn't report the active scene or music mode, so we retain the
/// last value applied by this bridge. Animated scenes naturally change color,
/// therefore color observations are not a reliable signal that a scene ended.
#[derive(Clone, Debug)]
struct ActiveSceneInfo {
    pub capability_instance: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct ActiveMusicModeInfo {
    pub mode: String,
    pub sensitivity: u32,
    pub auto_color: bool,
}

/// Represents the device state; synthesized from the various
/// sources of facts that we have in the Device
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceState {
    /// Whether the device is powered on
    pub on: bool,
    /// Whether the light function of the device is powered on
    pub light_on: Option<bool>,

    /// Whether the device is connected to the Govee cloud
    pub online: Option<bool>,

    /// The color temperature in kelvin
    pub kelvin: u32,

    /// The color
    pub color: crate::lan_api::DeviceColor,

    /// The brightness in percent (0-100)
    pub brightness: u8,

    /// The active effect mode, if known
    pub scene: Option<String>,

    /// Active work-mode number reported by AWS IoT, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<i64>,

    /// When `mode` was learned from the AWS IoT status message. The LAN and
    /// Platform API projections carry the last IoT mode forward, so their
    /// `updated` stamp says nothing about the mode's age; this field keeps
    /// the original observation time so consumers can judge staleness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_updated: Option<DateTime<Utc>>,

    /// Where the information came from
    pub source: &'static str,
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UndocDeviceInfo {
    pub room_name: Option<String>,
    pub entry: crate::undoc_api::DeviceEntry,
}

impl Device {
    /// Create a new device given just its sku and id.
    /// No other facts are known or reflected by it at this time;
    /// they will need to be added by the caller.
    pub fn new<S: Into<String>, I: Into<String>>(sku: S, id: I) -> Self {
        Self {
            sku: sku.into(),
            id: id.into(),
            ..Self::default()
        }
    }

    /// Returns the device name. Priority: user config override > Govee App name > computed name.
    pub fn name(&self) -> String {
        if let Some(ovr) = crate::service::device_config::get_device_override(&self.id, &self.sku) {
            if let Some(name) = ovr.name {
                return name;
            }
        }
        if let Some(name) = self.govee_name() {
            return name.to_string();
        }
        self.computed_name()
    }

    /// Returns the name defined for the device in the Govee App
    pub fn govee_name(&self) -> Option<&str> {
        if let Some(info) = &self.http_device_info {
            return Some(&info.device_name);
        }
        None
    }

    pub fn room_name(&self) -> Option<String> {
        if let Some(ovr) = crate::service::device_config::get_device_override(&self.id, &self.sku) {
            if let Some(room) = ovr.room {
                return Some(room);
            }
        }
        if let Some(info) = &self.undoc_device_info {
            return info.room_name.clone();
        }
        None
    }

    /// compute a name from the SKU and the last couple of bytes from the
    /// device id, similar to the device name that would show up in a BLE
    /// scan, or the default name for the device if not otherwise configured
    /// in the Govee App.
    pub fn computed_name(&self) -> String {
        // The id is usually "XX:XX:XX:XX:XX:XX:XX:XX" but some devices
        // report it without colons, and in lowercase.  Normalize it.
        let mut id = String::new();
        for c in self.id.chars() {
            if c == ':' {
                continue;
            }
            id.push(c.to_ascii_uppercase());
        }

        format!("{}_{}", self.sku, &id[id.len().saturating_sub(4)..])
    }

    pub fn preferred_poll_interval(&self) -> chrono::Duration {
        match self.device_type() {
            // If the kettle is on, read its temperature more frequently
            DeviceType::Kettle => {
                if self.device_state().map(|s| s.on).unwrap_or(false) {
                    chrono::Duration::seconds(60)
                } else {
                    *POLL_INTERVAL
                }
            }
            _ => *POLL_INTERVAL,
        }
    }

    /// Returns whether this device is currently reachable through at least one
    /// transport. Sending an IoT status request is deliberately not enough:
    /// `last_polled` is a throttle timestamp and does not prove that an offline
    /// device answered.
    pub fn is_online(&self, now: DateTime<Utc>) -> bool {
        let stale_threshold = self.preferred_poll_interval() * 3;

        if self
            .last_lan_device_status_update
            .map(|last_seen| now - last_seen < stale_threshold)
            .unwrap_or(false)
        {
            return true;
        }

        // An explicit cloud-offline result is authoritative until a later IoT,
        // Platform-state or cloud-control success calls `set_cloud_online(true)`.
        match self.cloud_online {
            Some(false) => false,
            Some(true) => self
                .last_cloud_status_update
                // The account device list refreshes roughly every ten minutes,
                // so use a wider window than the normal two-minute state poll.
                .map(|last_seen| now - last_seen < chrono::Duration::minutes(30))
                .unwrap_or(false),
            None => self
                .last_iot_device_status_update
                .or(self.last_http_device_state_update)
                .map(|last_seen| now - last_seen < stale_threshold)
                .unwrap_or(false),
        }
    }

    /// Record a positive or negative cloud-connectivity observation.
    pub fn set_cloud_online(&mut self, online: bool) {
        self.cloud_online = Some(online);
        self.last_cloud_status_update = Some(Utc::now());
    }

    /// Latest cloud status, independent of whichever transport supplied the
    /// most recent combined device state.
    pub fn cloud_online(&self) -> Option<bool> {
        self.cloud_online
    }

    pub fn cloud_probe_due(&self, now: DateTime<Utc>, interval: chrono::Duration) -> bool {
        self.last_cloud_probe_attempt
            .iter()
            .chain(self.last_cloud_status_update.iter())
            .max()
            .cloned()
            .map(|last| now - last >= interval)
            .unwrap_or(true)
    }

    pub fn mark_cloud_probe_attempt(&mut self) {
        self.last_cloud_probe_attempt = Some(Utc::now());
    }

    pub fn ip_addr(&self) -> Option<IpAddr> {
        self.lan_device.as_ref().map(|device| device.ip)
    }

    pub fn set_last_polled(&mut self) {
        self.last_polled.replace(Utc::now());
    }

    pub fn set_nightlight_state(&mut self, params: NotifyHumidifierNightlightParams) {
        self.nightlight_state.replace(params);
    }

    pub fn set_target_humidity(&mut self, percent: u8) {
        self.target_humidity_percent.replace(percent);
    }

    pub fn set_humidifier_work_mode_and_param(&mut self, mode: u8, param: u8) {
        self.humidifier_work_mode.replace(mode);
        self.humidifier_param_by_mode.insert(mode, param);
    }

    pub fn active_scene_name(&self) -> Option<&str> {
        self.active_scene.as_ref().map(|info| info.name.as_str())
    }

    pub fn active_scene_instance(&self) -> Option<&str> {
        self.active_scene
            .as_ref()
            .and_then(|info| info.capability_instance.as_deref())
    }

    pub fn scene_catalog_cache(&self) -> Option<&SceneCatalogCache> {
        self.scene_catalog_cache.as_ref()
    }

    pub fn set_scene_catalog(&mut self, catalog: SceneCatalogCache) {
        self.scene_catalog_cache = Some(catalog);
    }

    pub fn clear_scene_catalog(&mut self) {
        self.scene_catalog_cache = None;
    }

    pub fn active_music_mode(&self) -> Option<&ActiveMusicModeInfo> {
        self.active_music_mode.as_ref()
    }

    pub fn dreamview_enabled(&self) -> Option<bool> {
        self.dreamview_enabled
    }

    pub fn set_dreamview_enabled(&mut self, enabled: bool) {
        self.dreamview_enabled = Some(enabled);
        if enabled {
            // DreamView, scenes and music modes are mutually exclusive on the
            // device. Keep our optimistic state consistent with that behavior.
            self.active_scene.take();
            self.active_music_mode.take();
        }
    }

    /// Update the LAN device information
    pub fn set_lan_device(&mut self, device: LanDevice) {
        self.lan_device.replace(device);
        self.last_lan_device_update.replace(Utc::now());
    }

    /// Update the LAN device status information
    pub fn set_lan_device_status(&mut self, status: LanDeviceStatus) -> bool {
        let changed = self
            .lan_device_status
            .as_ref()
            .map(|prior| *prior != status)
            .unwrap_or(true);
        self.lan_device_status.replace(status);
        self.last_lan_device_status_update.replace(Utc::now());
        self.clear_scene_if_light_powered_off(self.compute_lan_device_state());
        changed
    }

    pub fn set_iot_device_status(&mut self, status: LanDeviceStatus) {
        self.iot_device_status.replace(status);
        self.last_iot_device_status_update.replace(Utc::now());
        self.set_cloud_online(true);
        self.clear_scene_if_light_powered_off(self.compute_iot_device_state());
    }

    pub fn set_http_device_info(&mut self, info: HttpDeviceInfo) {
        self.http_device_info.replace(info);
        self.last_http_device_update.replace(Utc::now());
    }

    /// Record connectivity and independently reported feature toggles from a
    /// Platform state response without changing the preferred device-state
    /// transport. Used by the background cloud-status probe for LAN devices.
    pub fn observe_http_device_state(&mut self, state: &HttpDeviceState) {
        let cloud_online = state
            .capability_by_instance("online")
            .and_then(|cap| cap.state.pointer("/value").and_then(|value| value.as_bool()))
            // A successful Platform state response is itself a positive cloud
            // observation when this SKU omits the optional online capability.
            .unwrap_or(true);
        if let Some(enabled) = state
            .capability_by_instance("dreamViewToggle")
            .and_then(|cap| cap.state.pointer("/value").and_then(|value| value.as_i64()))
            .map(|value| value != 0)
        {
            self.set_dreamview_enabled(enabled);
        }
        self.set_cloud_online(cloud_online);
    }

    pub fn set_http_device_state(&mut self, state: HttpDeviceState) {
        self.observe_http_device_state(&state);
        self.http_device_state.replace(state);
        self.last_http_device_state_update.replace(Utc::now());
        self.clear_scene_if_light_powered_off(self.compute_http_device_state());
    }

    pub fn set_undoc_device_info(
        &mut self,
        entry: crate::undoc_api::DeviceEntry,
        room_name: Option<&str>,
    ) {
        self.undoc_device_info.replace(UndocDeviceInfo {
            entry,
            room_name: room_name.map(|s| s.to_string()),
        });
        self.last_undoc_device_info_update.replace(Utc::now());
        // The account device-list endpoint frequently carries a stale
        // `lastDeviceData.online` value in either direction. Cloud status is
        // therefore learned only from a live IoT message or a direct Platform
        // state/control response, never from this cached account metadata.
    }

    pub fn compute_iot_device_state(&self) -> Option<DeviceState> {
        let updated = self.last_iot_device_status_update?;
        let status = self.iot_device_status.as_ref()?;

        Some(DeviceState {
            on: status.on,
            light_on: if self.device_type() == DeviceType::Light {
                Some(status.on)
            } else {
                self.nightlight_state.as_ref().map(|s| s.on)
            },
            online: None,
            brightness: status.brightness,
            color: status.color,
            kelvin: status.color_temperature_kelvin,
            scene: self.active_scene.as_ref().map(|info| info.name.to_string()),
            mode: status.mode,
            mode_updated: status.mode.and(self.last_iot_mode_update),
            source: "AWS IoT API",
            updated,
        })
    }

    pub fn compute_lan_device_state(&self) -> Option<DeviceState> {
        let updated = self.last_lan_device_status_update?;
        let status = self.lan_device_status.as_ref()?;

        // The LAN devStatus response doesn't carry a mode field; carry over
        // the last mode learned via AWS IoT, if any. Keep the original IoT
        // observation time in `mode_updated`: this projection's `updated` is
        // the LAN poll time, which would present an hours-old mode as fresh.
        let (mode, mode_updated) = match status.mode {
            Some(mode) => (Some(mode), Some(updated)),
            None => match self.iot_device_status.as_ref().and_then(|s| s.mode) {
                Some(mode) => (Some(mode), self.last_iot_mode_update),
                None => (None, None),
            },
        };

        Some(DeviceState {
            on: status.on,
            light_on: Some(status.on), // assumption: LAN API == light
            online: None,
            brightness: status.brightness,
            color: status.color,
            kelvin: status.color_temperature_kelvin,
            scene: self.active_scene.as_ref().map(|info| info.name.to_string()),
            mode,
            mode_updated,
            source: "LAN API",
            updated,
        })
    }

    pub fn compute_http_device_state(&self) -> Option<DeviceState> {
        let updated = self.last_http_device_state_update?;
        let state = self.http_device_state.as_ref()?;

        let mut online = None;
        let mut on = false;
        let mut light_on = None;
        let mut brightness = 0;
        let mut color = DeviceColor::default();
        let mut kelvin = 0;

        #[derive(serde::Deserialize)]
        struct IntegerValueState {
            value: u32,
        }
        #[derive(serde::Deserialize)]
        struct BoolValueState {
            value: bool,
        }

        let light_instance = self.get_light_power_toggle_instance_name();

        for cap in &state.capabilities {
            if let Ok(value) = serde_json::from_value::<IntegerValueState>(cap.state.clone()) {
                if light_instance
                    .as_deref()
                    .map(|inst| inst == cap.instance.as_str())
                    .unwrap_or(false)
                {
                    light_on.replace(value.value != 0);
                }

                match cap.instance.as_str() {
                    "powerSwitch" => {
                        on = value.value != 0;
                    }
                    "colorRgb" => {
                        color = DeviceColor {
                            r: ((value.value >> 16) & 0xff) as u8,
                            g: ((value.value >> 8) & 0xff) as u8,
                            b: (value.value & 0xff) as u8,
                        };
                    }
                    "brightness" => {
                        brightness = value.value as u8;
                    }
                    "colorTemperatureK" => {
                        kelvin = value.value;
                    }
                    _ => {}
                }
            } else if cap.instance == "online" {
                if let Ok(value) = serde_json::from_value::<BoolValueState>(cap.state.clone()) {
                    online.replace(value.value);
                }
            }
        }

        // The Platform API doesn't report a work mode for lights; carry over
        // the last mode learned via AWS IoT, if any, keeping the original IoT
        // observation time (see compute_lan_device_state).
        let mode = self.iot_device_status.as_ref().and_then(|s| s.mode);
        let mode_updated = mode.and(self.last_iot_mode_update);

        Some(DeviceState {
            on,
            light_on,
            online,
            brightness,
            color,
            kelvin,
            scene: self.active_scene.as_ref().map(|info| info.name.to_string()),
            mode,
            mode_updated,
            source: "PLATFORM API",
            updated,
        })
    }

    /// Returns the most recently received state information
    pub fn device_state(&self) -> Option<DeviceState> {
        let mut candidates = vec![];

        if let Some(state) = self.compute_lan_device_state() {
            candidates.push(state);
        }
        if let Some(state) = self.compute_http_device_state() {
            candidates.push(state);
        }
        if let Some(state) = self.compute_iot_device_state() {
            candidates.push(state);
        }

        candidates.sort_by(|a, b| a.updated.cmp(&b.updated));

        candidates.pop()
    }

    /// Records the active scene name
    pub fn set_active_scene(&mut self, scene: Option<&str>) {
        self.set_active_scene_for_instance(None, scene);
    }

    pub fn set_active_scene_for_instance(&mut self, instance: Option<&str>, scene: Option<&str>) {
        match scene {
            None => {
                self.active_scene.take();
                self.active_music_mode.take();
            }
            Some(scene) => {
                self.dreamview_enabled = Some(false);
                if instance != Some("musicMode") {
                    self.active_music_mode.take();
                }
                self.active_scene.replace(ActiveSceneInfo {
                    capability_instance: instance.map(str::to_string),
                    name: scene.to_string(),
                });
            }
        }
    }

    /// Power-off is an unambiguous end to a scene. Color changes are not:
    /// animated scenes intentionally produce a different color on every poll.
    fn clear_scene_if_light_powered_off(&mut self, source_state: Option<DeviceState>) {
        let is_light_off =
            source_state.map(|state| state.light_on.unwrap_or(state.on)) == Some(false);
        if is_light_off {
            self.active_scene.take();
            self.active_music_mode.take();
            self.dreamview_enabled = Some(false);
        }
    }

    pub fn set_active_music_mode(&mut self, mode: &str, sensitivity: u32, auto_color: bool) {
        self.active_music_mode.replace(ActiveMusicModeInfo {
            mode: mode.to_string(),
            sensitivity,
            auto_color,
        });
        self.set_active_scene_for_instance(Some("musicMode"), Some(&format!("Music: {mode}")));
    }

    pub fn update_active_music_mode(
        &mut self,
        sensitivity: Option<u32>,
        auto_color: Option<bool>,
    ) -> anyhow::Result<()> {
        let music = self
            .active_music_mode
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("music mode is not currently active"))?;

        if let Some(sensitivity) = sensitivity {
            music.sensitivity = sensitivity;
        }
        if let Some(auto_color) = auto_color {
            music.auto_color = auto_color;
        }
        Ok(())
    }

    pub fn device_type(&self) -> DeviceType {
        if let Some(info) = &self.http_device_info {
            info.device_type.clone()
        } else if let Some(q) = resolve_quirk(&self.sku) {
            q.device_type.clone()
        } else {
            DeviceType::Light
        }
    }

    /// Indicate whether we require the platform API data in order
    /// to correctly report the device
    pub fn needs_platform_poll(&self) -> bool {
        if !self.iot_api_supported() {
            return true;
        }

        let device_type = self.device_type();
        match (device_type, self.sku.as_str()) {
            (_, "H7160") => false,
            (DeviceType::Humidifier, _) => true,
            (DeviceType::Light, _) => false,
            (DeviceType::Kettle, _) => true,
            _ => true,
        }
    }

    pub fn pollable_via_lan(&self) -> bool {
        self.lan_device.is_some()
    }

    pub fn pollable_via_iot(&self) -> bool {
        if !self.iot_api_supported() {
            return false;
        }
        let device_type = self.device_type();
        match (device_type, self.sku.as_str()) {
            (_, "H7160") => true,
            (DeviceType::Light, _) => true,
            _ => false,
        }
    }

    pub fn avoid_platform_api(&self) -> bool {
        if let Some(ovr) = crate::service::device_config::get_device_override(&self.id, &self.sku) {
            if ovr.prefer_lan == Some(true) && self.lan_device.is_some() {
                return true;
            }
        }

        if let Some(quirk) = self.resolve_quirk() {
            if quirk.avoid_platform_api {
                return true;
            }
            if self.lan_device.is_some()
                && !self
                    .http_device_info
                    .as_ref()
                    .map(|info| info.supports_rgb())
                    .unwrap_or(false)
            {
                // Conflicting information:
                // Platform API says that this device isn't
                // a light, but the LAN API support suggests
                // that it is a light!
                // Therefore we will not trust the Platform API
                return true;
            }
        }
        false
    }

    pub fn resolve_quirk(&self) -> Option<Quirk> {
        match resolve_quirk(&self.sku) {
            Some(q) => Some(q.clone()),
            None => {
                // It's an unknown device, but since it showed up via LAN disco,
                // we can assume that it is a light
                if self.lan_device.is_some() {
                    Some(Quirk::light(Cow::Owned(self.sku.to_string()), BULB).with_lan_api())
                } else {
                    None
                }
            }
        }
    }

    pub fn get_capability_by_instance(&self, instance: &str) -> Option<&DeviceCapability> {
        self.http_device_info
            .as_ref()
            .and_then(|info| info.capability_by_instance(instance))
    }

    pub fn get_state_capability_by_instance(
        &self,
        instance: &str,
    ) -> Option<&DeviceCapabilityState> {
        self.http_device_state
            .as_ref()
            .and_then(|info| info.capability_by_instance(instance))
    }

    pub fn get_light_power_toggle_instance_name(&self) -> Option<&'static str> {
        match self.device_type() {
            DeviceType::Light => Some("powerSwitch"),
            _ => {
                // If the device's primary function is not a light,
                // then we need to avoid powering on its other function
                // here.  If it has a nightlight capability, that is
                // probably what we are controlling.
                // We may need to expand this to other power toggles
                // in the future.
                if self
                    .get_capability_by_instance("nightlightToggle")
                    .is_some()
                {
                    Some("nightlightToggle")
                } else {
                    None
                }
            }
        }
    }

    pub fn get_color_temperature_range(&self) -> Option<(u32, u32)> {
        // User config override takes highest priority
        if let Some(ovr) = crate::service::device_config::get_device_override(&self.id, &self.sku) {
            if let Some(range) = ovr.color_temp_range {
                return Some(range);
            }
        }

        if let Some(quirk) = self.resolve_quirk() {
            return quirk.color_temp_range;
        }

        if self.lan_device.is_some() {
            // LAN API support suggests that it is a light
            return Some((2000, 9000));
        }

        self.http_device_info
            .as_ref()
            .and_then(|info| info.get_color_temperature_range())
    }

    pub fn supports_brightness(&self) -> bool {
        if let Some(quirk) = self.resolve_quirk() {
            return quirk.supports_brightness;
        }

        if self.lan_device.is_some() {
            // LAN API support suggests that it is a light
            return true;
        }

        self.http_device_info
            .as_ref()
            .map(|info| info.supports_brightness())
            .unwrap_or(false)
    }

    pub fn iot_api_supported(&self) -> bool {
        // Quirks are explicit overrides and take precedence over
        // runtime auto-detection. A quirk with iot_api_supported=false
        // will disable IoT even if state updates have been received.
        if let Some(quirk) = self.resolve_quirk() {
            return quirk.iot_api_supported;
        }

        // If we've received IoT state updates for this device,
        // the IoT API is clearly working — use it for control too.
        if self.last_iot_device_status_update.is_some() {
            return true;
        }

        // The undocumented API reports whether the device has an IoT
        // topic, indicating it supports IoT control.
        if let Some(info) = &self.undoc_device_info {
            if info.entry.device_ext.device_settings.topic.is_some() {
                return true;
            }
        }

        false
    }

    pub fn supports_rgb(&self) -> bool {
        if let Some(quirk) = self.resolve_quirk() {
            return quirk.supports_rgb;
        }

        if self.lan_device.is_some() {
            // LAN API support suggests that it is a light
            return true;
        }

        self.http_device_info
            .as_ref()
            .map(|info| info.supports_rgb())
            .unwrap_or(false)
    }

    pub fn supports_dreamview(&self) -> bool {
        self.get_capability_by_instance("dreamViewToggle")
            .is_some()
            || self
                .resolve_quirk()
                .map(|quirk| quirk.supports_dreamview)
                .unwrap_or(false)
    }

    pub fn is_ble_only_device(&self) -> Option<bool> {
        if let Some(quirk) = self.resolve_quirk() {
            return Some(quirk.ble_only);
        }

        if self.http_device_info.is_some() {
            // truly BLE-only devices are not returned via the Platform API,
            // unless we have a quirk to say otherwise
            return Some(false);
        }

        if let Some(info) = &self.undoc_device_info {
            Some(info.entry.device_ext.device_settings.wifi_name.is_none())
        } else {
            // Don't know for sure
            None
        }
    }

    pub fn is_controllable(&self) -> bool {
        match self.is_ble_only_device() {
            Some(true) => false,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Device;
    use crate::lan_api::{DeviceColor, DeviceStatus};
    use crate::platform_api::{DeviceCapabilityKind, DeviceCapabilityState, HttpDeviceState};
    use serde_json::json;

    #[test]
    fn animated_scene_survives_color_changes_and_clears_when_powered_off() {
        let mut device = Device::new("H6000", "aa:bb");

        device.set_lan_device_status(DeviceStatus {
            on: true,
            brightness: 100,
            color: DeviceColor { r: 255, g: 0, b: 0 },
            color_temperature_kelvin: 0,
            mode: None,
        });

        device.set_active_scene(Some("Sunrise"));

        device.set_lan_device_status(DeviceStatus {
            on: true,
            brightness: 100,
            color: DeviceColor { r: 0, g: 0, b: 255 },
            color_temperature_kelvin: 0,
            mode: None,
        });

        assert_eq!(
            device.device_state().and_then(|state| state.scene),
            Some("Sunrise".to_string())
        );

        device.set_lan_device_status(DeviceStatus {
            on: false,
            brightness: 100,
            color: DeviceColor { r: 0, g: 255, b: 0 },
            color_temperature_kelvin: 0,
            mode: None,
        });

        assert_eq!(device.device_state().and_then(|state| state.scene), None);
    }

    #[test]
    fn music_mode_sets_active_scene_and_tracks_music_settings() {
        let mut device = Device::new("H6000", "aa:bb");

        device.set_active_music_mode("Spectrum", 77, false);

        assert_eq!(device.active_scene_name(), Some("Music: Spectrum"));
        assert_eq!(device.active_scene_instance(), Some("musicMode"));

        let music = device
            .active_music_mode()
            .expect("music mode should be set");
        assert_eq!(music.mode, "Spectrum");
        assert_eq!(music.sensitivity, 77);
        assert!(!music.auto_color);

        device
            .update_active_music_mode(Some(42), Some(true))
            .unwrap();
        let music = device
            .active_music_mode()
            .expect("music mode should remain set");
        assert_eq!(music.sensitivity, 42);
        assert!(music.auto_color);
    }

    #[test]
    fn non_music_scene_replaces_active_music_mode() {
        let mut device = Device::new("H6000", "aa:bb");
        device.set_active_music_mode("Rhythm", 100, true);

        device.set_active_scene_for_instance(Some("lightScene"), Some("Sunrise"));

        assert_eq!(device.active_scene_name(), Some("Sunrise"));
        assert_eq!(device.active_scene_instance(), Some("lightScene"));
        assert!(device.active_music_mode().is_none());
    }

    #[test]
    fn dreamview_state_is_optimistic_and_mutually_exclusive_with_scenes() {
        let mut device = Device::new("H66A1", "aa:bb");
        device.set_active_scene(Some("Sunrise"));

        device.set_dreamview_enabled(true);
        assert_eq!(device.dreamview_enabled(), Some(true));
        assert!(device.active_scene_name().is_none());
        assert!(device.active_music_mode().is_none());

        device.set_active_scene(Some("Ocean"));
        assert_eq!(device.dreamview_enabled(), Some(false));
        assert_eq!(device.active_scene_name(), Some("Ocean"));
    }

    #[test]
    fn verified_models_report_dreamview_hardware_support_without_cloud_metadata() {
        for sku in ["H66A1", "H6199"] {
            let device = Device::new(sku, "aa:bb");
            assert!(device.supports_dreamview(), "{sku}");
        }
    }

    #[test]
    fn platform_state_reconciles_optimistic_dreamview_state() {
        let mut device = Device::new("H66A1", "aa:bb");
        device.set_dreamview_enabled(true);

        device.set_http_device_state(HttpDeviceState {
            sku: "H66A1".to_string(),
            device: "aa:bb".to_string(),
            capabilities: vec![DeviceCapabilityState {
                kind: DeviceCapabilityKind::Toggle,
                instance: "dreamViewToggle".to_string(),
                state: json!({"value": 0}),
            }],
        });

        assert_eq!(device.dreamview_enabled(), Some(false));
    }

    #[test]
    fn cloud_probe_observation_does_not_replace_lan_state() {
        let mut device = Device::new("H66A1", "aa:bb");
        device.set_lan_device_status(DeviceStatus {
            on: true,
            brightness: 80,
            color: DeviceColor { r: 1, g: 2, b: 3 },
            color_temperature_kelvin: 4000,
            mode: None,
        });
        let platform_state = HttpDeviceState {
            sku: "H66A1".to_string(),
            device: "aa:bb".to_string(),
            capabilities: vec![DeviceCapabilityState {
                kind: DeviceCapabilityKind::Toggle,
                instance: "dreamViewToggle".to_string(),
                state: json!({"value": 1}),
            }],
        };

        device.observe_http_device_state(&platform_state);

        assert_eq!(device.device_state().unwrap().source, "LAN API");
        assert_eq!(device.cloud_online(), Some(true));
        assert_eq!(device.dreamview_enabled(), Some(true));
        assert!(device.http_device_state.is_none());
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn name_compute() {
        let device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        assert_eq!(device.name(), "H6000_422A");

        let device = Device::new("H6127", "cef142b0b354995f");
        assert_eq!(device.name(), "H6127_995F");

        let device = Device::new("H6127", "ce");
        assert_eq!(device.name(), "H6127_CE");
    }

    #[test]
    fn is_online_false_when_never_seen() {
        let device = Device::new("H6000", "aa:bb");
        assert!(!device.is_online(Utc::now()));
    }

    #[test]
    fn is_online_false_when_only_a_poll_request_was_sent() {
        let mut device = Device::new("H6000", "aa:bb");
        device.last_polled = Some(Utc::now());
        assert!(!device.is_online(Utc::now()));
    }

    #[test]
    fn is_online_true_from_lan_update() {
        let mut device = Device::new("H6000", "aa:bb");
        device.last_lan_device_status_update = Some(Utc::now());
        assert!(device.is_online(Utc::now()));
    }

    #[test]
    fn is_online_true_from_iot_update() {
        let mut device = Device::new("H6000", "aa:bb");
        device.last_iot_device_status_update = Some(Utc::now());
        assert!(device.is_online(Utc::now()));
    }

    #[test]
    fn is_online_true_from_http_update() {
        let mut device = Device::new("H6000", "aa:bb");
        device.last_http_device_state_update = Some(Utc::now());
        assert!(device.is_online(Utc::now()));
    }

    #[test]
    fn explicit_cloud_status_controls_cloud_only_availability() {
        let mut device = Device::new("H6000", "aa:bb");

        device.set_cloud_online(true);
        assert_eq!(device.cloud_online(), Some(true));
        assert!(device.is_online(Utc::now()));

        device.set_cloud_online(false);
        assert_eq!(device.cloud_online(), Some(false));
        assert!(!device.is_online(Utc::now()));
    }

    #[test]
    fn cloud_probe_is_rate_limited_independently_from_state_polling() {
        let mut device = Device::new("H6000", "aa:bb");
        let now = Utc::now();
        let interval = chrono::Duration::minutes(15);

        assert!(device.cloud_probe_due(now.clone(), interval));
        device.mark_cloud_probe_attempt();
        assert!(!device.cloud_probe_due(now.clone(), interval));
        assert!(device.cloud_probe_due(
            now + interval + chrono::Duration::seconds(1),
            interval
        ));
    }

    #[test]
    fn recent_lan_response_remains_available_when_cloud_is_offline() {
        let mut device = Device::new("H6000", "aa:bb");
        device.last_lan_device_status_update = Some(Utc::now());
        device.set_cloud_online(false);

        assert!(device.is_online(Utc::now()));
    }

    #[test]
    fn is_online_false_when_stale() {
        let mut device = Device::new("H6000", "aa:bb");
        // Last seen 10 minutes ago; default interval is 120s, stale threshold = 360s
        device.last_polled = Some(Utc::now() - chrono::Duration::seconds(600));
        assert!(!device.is_online(Utc::now()));
    }

    #[test]
    fn iot_api_supported_false_by_default() {
        let device = Device::new("H6000", "aa:bb");
        assert!(!device.iot_api_supported());
    }

    #[test]
    fn iot_api_supported_true_when_iot_status_received() {
        let mut device = Device::new("H6000", "aa:bb");
        device.last_iot_device_status_update = Some(Utc::now());
        assert!(device.iot_api_supported());
    }

    #[test]
    fn iot_api_supported_true_when_undoc_topic_present() {
        let mut device = Device::new("H6072", "aa:bb");
        let resp: crate::undoc_api::DevicesResponse =
            crate::platform_api::from_json(include_str!("../../test-data/undoc-device-list.json"))
                .unwrap();
        // First device in the test data has topic: "GD/"
        let entry = resp.devices.into_iter().next().unwrap();
        assert!(entry.device_ext.device_settings.topic.is_some());
        device.undoc_device_info = Some(UndocDeviceInfo {
            room_name: None,
            entry,
        });
        assert!(device.iot_api_supported());
    }

    #[test]
    fn undocumented_account_offline_status_is_not_authoritative() {
        let resp: crate::undoc_api::DevicesResponse =
            crate::platform_api::from_json(include_str!("../../test-data/undoc-device-list.json"))
                .unwrap();
        let entry = resp
            .devices
            .into_iter()
            .find(|entry| entry.device_ext.last_device_data.online == Some(false))
            .unwrap();
        let mut device = Device::new(&entry.sku, &entry.device);

        device.set_undoc_device_info(entry, None);

        assert_eq!(device.cloud_online(), None);
    }

    #[test]
    fn undocumented_account_online_status_is_not_authoritative() {
        let resp: crate::undoc_api::DevicesResponse = crate::platform_api::from_json(include_str!(
            "../../test-data/undoc-device-list-issue-21.json"
        ))
        .unwrap();
        let entry = resp
            .devices
            .into_iter()
            .find(|entry| entry.device_ext.last_device_data.online == Some(true))
            .unwrap();
        let mut device = Device::new(&entry.sku, &entry.device);

        device.set_undoc_device_info(entry, None);

        assert_eq!(device.cloud_online(), None);
    }

    #[test]
    fn platform_not_belong_until_defaults_to_none() {
        let device = Device::new("H6000", "aa:bb");
        assert!(device.platform_not_belong_until.is_none());
    }

    fn status_with_mode(mode: Option<i64>) -> LanDeviceStatus {
        LanDeviceStatus {
            on: true,
            brightness: 100,
            color: DeviceColor { r: 255, g: 0, b: 0 },
            color_temperature_kelvin: 0,
            mode,
        }
    }

    /// Regression guard for the mode field itself: a `mode` learned from an
    /// AWS IoT status must survive into the synthesized `DeviceState`,
    /// stamped with the IoT observation time.
    #[test]
    fn iot_status_mode_reaches_device_state() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        // Mirror the subscriber merge: the status cache and the mode
        // observation are stamped together when the packet carries a mode.
        device.set_iot_device_status(status_with_mode(Some(5)));
        device.last_iot_mode_update = device.last_iot_device_status_update;

        let state = device.device_state().expect("iot state");
        assert_eq!(state.mode, Some(5));
        assert!(state.mode_updated.is_some());
        assert_eq!(state.mode_updated, device.last_iot_mode_update);
    }

    /// The LAN devStatus response has no mode field; the projection carries
    /// the last IoT-learned mode forward even when the LAN state is newer
    /// and wins the source race.
    #[test]
    fn iot_mode_carries_over_into_newer_lan_projection() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_iot_device_status(status_with_mode(Some(4)));
        device.set_lan_device_status(status_with_mode(None));
        device.last_lan_device_status_update = Some(Utc::now() + chrono::Duration::seconds(5));

        let state = device.device_state().expect("lan state");
        assert_eq!(state.source, "LAN API");
        assert_eq!(state.mode, Some(4));
    }

    /// The carry-over must not launder the mode's age. Observed live
    /// (2026-08-13, H607C): a LAN projection re-stamped a 10-hour-old IoT
    /// mode with a seconds-old `updated`. `mode_updated` has to keep the
    /// original IoT observation time so consumers can judge staleness.
    #[test]
    fn lan_carry_over_preserves_iot_mode_observation_time() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_iot_device_status(status_with_mode(Some(4)));
        let aged = Utc::now() - chrono::Duration::hours(10);
        device.last_iot_mode_update = Some(aged);

        device.set_lan_device_status(status_with_mode(None));

        let state = device.device_state().expect("lan state");
        assert_eq!(state.source, "LAN API");
        assert_eq!(state.mode, Some(4));
        assert_eq!(state.mode_updated, Some(aged));
        assert!(state.updated > aged + chrono::Duration::hours(9));
    }

    /// Same guarantee for the Platform API projection, which reports no work
    /// mode for lights either.
    #[test]
    fn platform_projection_preserves_iot_mode_observation_time() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_iot_device_status(status_with_mode(Some(4)));
        let aged = Utc::now() - chrono::Duration::hours(10);
        device.last_iot_mode_update = Some(aged);

        device.set_http_device_state(HttpDeviceState {
            sku: "H607C".to_string(),
            device: "AA:BB:CC:DD:EE:FF:42:2A".to_string(),
            capabilities: vec![],
        });

        let state = device.compute_http_device_state().expect("http state");
        assert_eq!(state.mode, Some(4));
        assert_eq!(state.mode_updated, Some(aged));
    }

    /// Without any IoT report the projections must not invent a mode or an
    /// observation time.
    #[test]
    fn no_iot_report_means_no_mode_and_no_timestamp() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_lan_device_status(status_with_mode(None));

        let state = device.device_state().expect("lan state");
        assert_eq!(state.mode, None);
        assert_eq!(state.mode_updated, None);
    }
}
