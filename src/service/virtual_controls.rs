use crate::service::device::{Device, DeviceState};
use crate::undoc_api::OneClickComponent;
use chrono::Utc;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};

/// Platform API objects that control a collection of physical devices rather
/// than representing an independently pollable device.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualDeviceKind {
    BaseGroup,
    DreamViewScene,
    MusicDreamView,
}

impl VirtualDeviceKind {
    pub fn from_sku(sku: &str) -> Option<Self> {
        match sku {
            "BaseGroup" => Some(Self::BaseGroup),
            // Scenic DreamView is operationally a DreamView mode: its real
            // state/control lives on the selected physical sync center's
            // dreamViewToggle capability, not on the virtual powerSwitch.
            "DreamViewScenic" | "MusicDreamView" => Some(Self::MusicDreamView),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualControlDefinition {
    pub kind: VirtualDeviceKind,
    /// IDs used by different generations of the Govee APIs for this object.
    pub ids: Vec<String>,
    pub name: String,
    pub member_ids: Vec<String>,
    /// Physical sync center used to activate API-backed DreamView modes.
    pub control_device_id: Option<String>,
}

impl VirtualControlDefinition {
    pub fn stable_id(&self) -> String {
        let identity = self
            .ids
            .first()
            .map(String::as_str)
            .filter(|id| !id.is_empty())
            .unwrap_or(&self.name);
        format!("{:?}-{identity}", self.kind)
    }
}

fn push_unique_case_insensitive(values: &mut Vec<String>, value: String) {
    if !value.is_empty()
        && !values
            .iter()
            .any(|prior| prior.eq_ignore_ascii_case(&value))
    {
        values.push(value);
    }
}

fn json_identifier(value: Option<&JsonValue>) -> Option<String> {
    match value? {
        JsonValue::String(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
        JsonValue::Number(value) => {
            let value = value.as_i64()?;
            (value > 0).then(|| value.to_string())
        }
        _ => None,
    }
}

fn has_saved_control_id(value: &JsonValue) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    ["gId", "groupId", "feastId", "id"]
        .iter()
        .any(|field| json_identifier(object.get(*field)).is_some())
}

fn device_id(device: &JsonValue) -> Option<String> {
    let value = device
        .get("device")
        .or_else(|| device.get("deviceId"))
        .unwrap_or(device);
    json_identifier(Some(value))
}

fn is_device_collection_field(name: &str) -> bool {
    matches!(
        name,
        "devices" | "subDevices" | "members" | "memberDevices" | "deviceList"
    )
}

/// Govee has moved membership between `devices`, `subDevices`, nested config
/// objects and direct arrays across app/API generations. Collect explicit
/// `device`/`deviceId` values everywhere and scalar IDs only when they are
/// directly inside a field that is known to be a device collection.
fn append_nested_device_ids(member_ids: &mut Vec<String>, value: &JsonValue) {
    fn walk(member_ids: &mut Vec<String>, value: &JsonValue, scalar_is_device: bool) {
        match value {
            JsonValue::Object(object) => {
                if let Some(id) = json_identifier(object.get("device"))
                    .or_else(|| json_identifier(object.get("deviceId")))
                {
                    push_unique_case_insensitive(member_ids, id);
                }
                for (name, child) in object {
                    walk(member_ids, child, is_device_collection_field(name));
                }
            }
            JsonValue::Array(values) => {
                for child in values {
                    walk(member_ids, child, scalar_is_device);
                }
            }
            JsonValue::String(_) | JsonValue::Number(_) if scalar_is_device => {
                if let Some(id) = json_identifier(Some(value)) {
                    push_unique_case_insensitive(member_ids, id);
                }
            }
            _ => {}
        }
    }

    walk(member_ids, value, false);
}

fn definition_from_json(
    value: &JsonValue,
    kind: VirtualDeviceKind,
    component_id: Option<u64>,
    component_main_device: Option<&JsonValue>,
    include_component_id: bool,
) -> Option<VirtualControlDefinition> {
    let object = value.as_object()?;
    let name = object
        .get("name")
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut ids = vec![];
    for field in ["gId", "groupId", "feastId", "presetId", "id"] {
        if let Some(id) = json_identifier(object.get(field)) {
            push_unique_case_insensitive(&mut ids, id);
        }
    }
    if include_component_id {
        if let Some(component_id) = component_id.filter(|id| *id > 0) {
            push_unique_case_insensitive(&mut ids, component_id.to_string());
        }
    }

    let entry_main_device = object.get("feastMainDevice").and_then(device_id);
    let component_main_device_id = component_main_device.and_then(device_id);
    let control_device_id = entry_main_device.or(component_main_device_id);

    let mut member_ids = vec![];
    append_nested_device_ids(&mut member_ids, value);
    if let Some(main_device) = component_main_device {
        // Some Govee Home versions keep the full Scenic/Music topology under
        // mainDevice.subDevices rather than in the feast entry itself.
        append_nested_device_ids(&mut member_ids, main_device);
    }
    if let Some(main) = control_device_id.as_ref() {
        push_unique_case_insensitive(&mut member_ids, main.clone());
    }

    (!name.is_empty() || !ids.is_empty() || !member_ids.is_empty()).then_some(
        VirtualControlDefinition {
            kind,
            ids,
            name,
            member_ids,
            control_device_id,
        },
    )
}

fn entry_feast_type(entry: &JsonValue, fallback: Option<u64>) -> Option<u64> {
    entry
        .get("feastType")
        .and_then(JsonValue::as_u64)
        .or(fallback)
}

fn is_generic_music_placeholder(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "music dreamview" | "music dream view" | "music"
    )
}

/// Build one account-level video DreamView mode. Govee may repeat
/// `feastMainDevice` inside member/environment entries; those objects are
/// members/configuration, not additional selected video sync centers.
fn video_dreamview_definition(
    component: &OneClickComponent,
    entries: &[&JsonValue],
) -> Option<VirtualControlDefinition> {
    let control_device_id = component
        .main_device
        .as_ref()
        .and_then(device_id)
        .or_else(|| {
            entries.iter().find_map(|entry| {
                entry
                    .get("feastMainDevice")
                    .and_then(device_id)
            })
        })?;

    let mut member_ids = vec![];
    if let Some(main) = component.main_device.as_ref() {
        append_nested_device_ids(&mut member_ids, main);
    }
    for entry in entries {
        append_nested_device_ids(&mut member_ids, entry);
    }
    push_unique_case_insensitive(&mut member_ids, control_device_id.clone());

    let mut ids = vec![];
    if component.component_id > 0 {
        ids.push(component.component_id.to_string());
    }

    Some(VirtualControlDefinition {
        // Existing API/UI plumbing already treats MusicDreamView as a
        // sync-center-backed mode rather than a pollable virtual power object.
        // Video and Scenic DreamView intentionally use that same operational
        // kind so they share cloud state/control without physical switches.
        kind: VirtualDeviceKind::MusicDreamView,
        ids,
        name: "DreamView".to_string(),
        member_ids,
        control_device_id: Some(control_device_id),
    })
}

/// Extract BaseGroup and all DreamView modes from the undocumented home-layout
/// response. Govee uses `feastType` 1 for Music DreamView, 2 for ScenicView and
/// 3 for video DreamView. All DreamView modes are controlled through the
/// selected physical sync center's `dreamViewToggle`; the physical device is
/// therefore not itself exposed as a DreamView switch.
pub fn parse_virtual_controls(
    components: &[OneClickComponent],
) -> Vec<VirtualControlDefinition> {
    let mut definitions = vec![];

    for component in components {
        for group in &component.groups {
            if let Some(definition) = definition_from_json(
                group,
                VirtualDeviceKind::BaseGroup,
                Some(component.component_id),
                component.main_device.as_ref(),
                component.groups.len() == 1,
            ) {
                definitions.push(definition);
            }
        }

        if component.component_type != 2 {
            continue;
        }

        let dreamview_entries: Vec<_> = component
            .feasts
            .iter()
            .chain(component.environments.iter())
            .collect();

        let has_video_dreamview = component.feast_type == Some(3)
            || dreamview_entries
                .iter()
                .any(|entry| entry_feast_type(entry, None) == Some(3));
        if has_video_dreamview {
            let video_entries: Vec<_> = dreamview_entries
                .iter()
                .copied()
                .filter(|entry| entry_feast_type(entry, component.feast_type) == Some(3))
                .collect();
            if let Some(definition) = video_dreamview_definition(component, &video_entries) {
                definitions.push(definition);
            }
        }

        for entry in &dreamview_entries {
            let feast_type = entry_feast_type(entry, component.feast_type);
            // Video DreamView is represented by the single account-level
            // definition above; do not turn each environment into a center.
            if feast_type == Some(3) {
                continue;
            }
            if !matches!(feast_type, Some(1) | Some(2)) {
                continue;
            }

            if let Some(definition) = definition_from_json(
                entry,
                VirtualDeviceKind::MusicDreamView,
                Some(component.component_id),
                component.main_device.as_ref(),
                dreamview_entries.len() == 1,
            ) {
                // Only suppress the unnamed/default Music DreamView placeholder.
                // User-saved cards remain valid even when this API generation
                // returns feastId=-1 and only the center.
                let empty_music_placeholder = feast_type == Some(1)
                    && !has_saved_control_id(entry)
                    && definition.member_ids.len() <= 1
                    && is_generic_music_placeholder(&definition.name);
                if !empty_music_placeholder {
                    definitions.push(definition);
                }
            }
        }
    }

    definitions
}

/// IDs of the actual user-selected video DreamView sync centers. Use only the
/// component's main device (or one fallback when it is absent); repeated
/// feast/environment `feastMainDevice` objects must never create extra centers.
pub fn parse_video_dreamview_centers(components: &[OneClickComponent]) -> HashSet<String> {
    let mut centers = HashSet::new();
    for component in components {
        if component.component_type != 2 {
            continue;
        }
        let entries: Vec<_> = component
            .feasts
            .iter()
            .chain(component.environments.iter())
            .collect();
        let is_video = component.feast_type == Some(3)
            || entries
                .iter()
                .any(|entry| entry_feast_type(entry, None) == Some(3));
        if !is_video {
            continue;
        }

        let center = component
            .main_device
            .as_ref()
            .and_then(device_id)
            .or_else(|| {
                entries.iter().find_map(|entry| {
                    (entry_feast_type(entry, component.feast_type) == Some(3))
                        .then(|| entry.get("feastMainDevice").and_then(device_id))
                        .flatten()
                })
            });
        if let Some(center) = center {
            centers.insert(center.to_ascii_lowercase());
        }
    }
    centers
}

pub fn definition_matches_device(
    definition: &VirtualControlDefinition,
    device: &Device,
) -> bool {
    VirtualDeviceKind::from_sku(&device.sku) == Some(definition.kind)
        && (definition
            .ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case(&device.id))
            || (!definition.name.is_empty()
                && definition.name.eq_ignore_ascii_case(&device.name())))
}

fn find_device<'a>(devices: &'a HashMap<String, Device>, id: &str) -> Option<&'a Device> {
    devices.get(id).or_else(|| {
        devices
            .values()
            .find(|device| device.id.eq_ignore_ascii_case(id))
    })
}

/// State for account-level DreamView modes comes from the selected sync
/// center's cloud `dreamViewToggle`. Base groups still aggregate member power.
pub fn aggregate_virtual_state(
    definition: &VirtualControlDefinition,
    devices: &HashMap<String, Device>,
) -> Option<DeviceState> {
    if definition.kind == VirtualDeviceKind::MusicDreamView {
        let center_id = definition.control_device_id.as_ref()?;
        let center = find_device(devices, center_id)?;
        let enabled = center.dreamview_enabled()?;
        let mut state = center.device_state()?;
        state.on = enabled;
        state.light_on = Some(enabled);
        state.source = "PLATFORM MODE";
        return Some(state);
    }

    if definition.member_ids.is_empty() {
        return None;
    }

    let mut seen = HashSet::new();
    let mut states = vec![];
    let mut missing_state = false;

    for member_id in &definition.member_ids {
        let key = member_id.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let Some(device) = find_device(devices, member_id) else {
            missing_state = true;
            continue;
        };
        let Some(state) = device.device_state() else {
            missing_state = true;
            continue;
        };
        states.push(state);
    }

    if states.is_empty() {
        return None;
    }

    let any_off = states.iter().any(|state| !state.on);
    if !any_off && missing_state {
        return None;
    }
    let on = !any_off;
    let brightness = (states
        .iter()
        .map(|state| state.brightness as u32)
        .sum::<u32>()
        / states.len() as u32) as u8;
    let first = states.first().expect("states is not empty");
    let updated = states
        .iter()
        .map(|state| &state.updated)
        .max()
        .cloned()
        .unwrap_or_else(Utc::now);
    let online = if states.iter().any(|state| state.online == Some(false)) {
        Some(false)
    } else if states.iter().all(|state| state.online == Some(true)) {
        Some(true)
    } else {
        None
    };

    Some(DeviceState {
        on,
        light_on: Some(on),
        online,
        kelvin: first.kelvin,
        color: first.color,
        brightness,
        scene: None,
        mode: None,
        mode_updated: None,
        source: "GROUP AGGREGATE",
        updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lan_api::{DeviceColor, DeviceStatus as LanDeviceStatus};

    fn device(id: &str, on: bool) -> Device {
        let mut device = Device::new("H6000", id);
        device.set_lan_device_status(LanDeviceStatus {
            on,
            brightness: 80,
            color: DeviceColor { r: 1, g: 2, b: 3 },
            color_temperature_kelvin: 4000,
            mode: None,
        });
        device
    }

    #[test]
    fn parses_base_group_and_scenic_members_without_names_hardcoded() {
        let response: crate::undoc_api::OneClickResponse =
            crate::platform_api::from_json(include_str!("../../test-data/undoc-one-click.json"))
                .unwrap();
        let definitions = parse_virtual_controls(&response.data.components);

        let group = definitions
            .iter()
            .find(|definition| definition.kind == VirtualDeviceKind::BaseGroup)
            .unwrap();
        assert!(group.ids.iter().any(|id| id == "1234"));
        assert_eq!(group.member_ids, vec![":35:29"]);

        let scenic = definitions
            .iter()
            .find(|definition| definition.name == "Scenic DreamView")
            .unwrap();
        assert_eq!(scenic.kind, VirtualDeviceKind::MusicDreamView);
        assert!(scenic.member_ids.iter().any(|id| id == ":11"));
        assert_eq!(scenic.control_device_id.as_deref(), Some(":11"));
    }

    #[test]
    fn aggregate_is_on_only_when_every_member_is_on() {
        let definition = VirtualControlDefinition {
            kind: VirtualDeviceKind::BaseGroup,
            ids: vec!["42".into()],
            name: "All lights".into(),
            member_ids: vec!["one".into(), "two".into()],
            control_device_id: None,
        };
        let mut devices = HashMap::new();
        devices.insert("one".into(), device("one", true));
        devices.insert("two".into(), device("two", true));
        assert!(aggregate_virtual_state(&definition, &devices).unwrap().on);

        devices.insert("two".into(), device("two", false));
        assert!(!aggregate_virtual_state(&definition, &devices).unwrap().on);
    }

    #[test]
    fn incomplete_all_on_group_remains_unknown() {
        let definition = VirtualControlDefinition {
            kind: VirtualDeviceKind::BaseGroup,
            ids: vec![],
            name: "Scene".into(),
            member_ids: vec!["one".into(), "missing".into()],
            control_device_id: None,
        };
        let mut devices = HashMap::new();
        devices.insert("one".into(), device("one", true));
        assert!(aggregate_virtual_state(&definition, &devices).is_none());
    }

    #[test]
    fn dreamview_mode_uses_center_cloud_toggle_not_member_power() {
        let definition = VirtualControlDefinition {
            kind: VirtualDeviceKind::MusicDreamView,
            ids: vec!["42".into()],
            name: "ScenicView".into(),
            member_ids: vec!["CENTER".into(), "MEMBER".into()],
            control_device_id: Some("CENTER".into()),
        };
        let mut center = device("CENTER", true);
        center.set_dreamview_enabled(false);
        let mut devices = HashMap::new();
        devices.insert("CENTER".into(), center);
        devices.insert("MEMBER".into(), device("MEMBER", true));

        let state = aggregate_virtual_state(&definition, &devices).unwrap();
        assert!(!state.on);
        assert_eq!(state.source, "PLATFORM MODE");
    }

    fn dreamview_component(
        feast_type: u64,
        main_device: JsonValue,
        feasts: Vec<JsonValue>,
    ) -> OneClickComponent {
        OneClickComponent {
            can_disable: None,
            can_manage: true,
            feast_type: Some(feast_type),
            feasts,
            groups: vec![],
            main_device: Some(main_device),
            component_id: 99,
            environments: vec![],
            name: "DreamView".into(),
            component_type: 2,
            guide_url: None,
            h5_url: None,
            video_url: None,
            one_clicks: vec![],
        }
    }

    #[test]
    fn music_dreamview_is_a_virtual_scene_with_recursive_members() {
        let components = vec![dreamview_component(
            1,
            serde_json::json!({"device": "CENTER"}),
            vec![serde_json::json!({
                "feastId": 1447,
                "name": "Saved Music Scene",
                "config": {
                    "devices": [
                        {"device": "CENTER"},
                        {"device": "MEMBER-1"},
                        {"device": "MEMBER-2"}
                    ]
                }
            })],
        )];

        let definitions = parse_virtual_controls(&components);
        assert_eq!(definitions.len(), 1);
        let music = &definitions[0];
        assert_eq!(music.kind, VirtualDeviceKind::MusicDreamView);
        assert_eq!(music.name, "Saved Music Scene");
        assert_eq!(music.control_device_id.as_deref(), Some("CENTER"));
        assert_eq!(music.member_ids, vec!["CENTER", "MEMBER-1", "MEMBER-2"]);
    }

    #[test]
    fn video_dreamview_is_one_virtual_mode_with_only_real_center_selected() {
        let mut component = dreamview_component(
            3,
            serde_json::json!({
                "device": "VIDEO-CENTER",
                "subDevices": [
                    {"device": "MEMBER-1"},
                    {"device": "MEMBER-2"}
                ]
            }),
            vec![serde_json::json!({
                "feastType": 3,
                "feastMainDevice": {"device": "NOT-A-CENTER"},
                "devices": [{"device": "MEMBER-3"}]
            })],
        );
        component.environments = vec![serde_json::json!({
            "feastType": 3,
            "feastMainDevice": {"device": "ALSO-NOT-A-CENTER"}
        })];
        let components = vec![component];

        let definitions = parse_virtual_controls(&components);
        let video = definitions
            .iter()
            .find(|definition| definition.name == "DreamView")
            .unwrap();
        assert_eq!(video.kind, VirtualDeviceKind::MusicDreamView);
        assert_eq!(video.control_device_id.as_deref(), Some("VIDEO-CENTER"));
        assert!(video.member_ids.iter().any(|id| id == "MEMBER-1"));
        assert!(video.member_ids.iter().any(|id| id == "MEMBER-3"));

        let centers = parse_video_dreamview_centers(&components);
        assert_eq!(centers.len(), 1);
        assert!(centers.contains("video-center"));
        assert!(!centers.contains("not-a-center"));
    }

    #[test]
    fn empty_generic_music_dreamview_placeholder_is_not_exposed() {
        let components = vec![dreamview_component(
            1,
            serde_json::json!({"device": "CENTER"}),
            vec![serde_json::json!({
                "feastId": -1,
                "name": "Music DreamView",
                "devices": []
            })],
        )];

        assert!(parse_virtual_controls(&components).is_empty());
    }

    #[test]
    fn saved_single_device_music_dreamview_is_exposed() {
        let components = vec![dreamview_component(
            1,
            serde_json::json!({"device": "CENTER"}),
            vec![serde_json::json!({
                "feastId": 1447,
                "name": "Solo Music",
                "devices": []
            })],
        )];

        let definitions = parse_virtual_controls(&components);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].member_ids, vec!["CENTER"]);
    }
}
