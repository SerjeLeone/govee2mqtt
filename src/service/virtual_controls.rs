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
            "DreamViewScenic" => Some(Self::DreamViewScene),
            "MusicDreamView" => Some(Self::MusicDreamView),
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
    let value = device.get("device").unwrap_or(device);
    json_identifier(Some(value))
}

/// Govee has moved the member list between `devices` and nested configuration
/// objects across app/API generations. Collect every object field literally
/// named `device`; unrelated identifiers such as `deviceType` are ignored.
fn append_nested_device_ids(member_ids: &mut Vec<String>, value: &JsonValue) {
    match value {
        JsonValue::Object(object) => {
            if let Some(id) = json_identifier(object.get("device")) {
                push_unique_case_insensitive(member_ids, id);
            }
            for child in object.values() {
                append_nested_device_ids(member_ids, child);
            }
        }
        JsonValue::Array(values) => {
            for child in values {
                append_nested_device_ids(member_ids, child);
            }
        }
        _ => {}
    }
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
    let component_main_device = component_main_device.and_then(device_id);
    let control_device_id = entry_main_device.or(component_main_device);

    let mut member_ids = vec![];
    append_nested_device_ids(&mut member_ids, value);
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

/// Extract BaseGroup, ScenicView and Music DreamView membership from the
/// undocumented home-layout response. Govee uses `feastType` 1 for Music
/// DreamView, 2 for ScenicView and 3 for video DreamView. Video DreamView is
/// represented by a switch on its selected physical sync center rather than a
/// second virtual object.
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

        let dreamview_entries: Vec<_> = component
            .feasts
            .iter()
            .chain(component.environments.iter())
            .collect();
        if component.component_type == 2 {
            let kind = match component.feast_type {
                Some(1) => Some(VirtualDeviceKind::MusicDreamView),
                Some(2) => Some(VirtualDeviceKind::DreamViewScene),
                // `3` is video DreamView and is exposed on the selected sync
                // center. Unknown subtypes must not be mislabeled as scenes.
                _ => None,
            };
            let Some(kind) = kind else {
                continue;
            };
            for entry in &dreamview_entries {
                if let Some(definition) = definition_from_json(
                    entry,
                    kind,
                    Some(component.component_id),
                    component.main_device.as_ref(),
                    dreamview_entries.len() == 1,
                ) {
                    // Empty/default Music DreamView placeholders have no
                    // positive saved ID and contain only the center. A valid
                    // one-device saved card must still be exposed.
                    let empty_music_placeholder = kind == VirtualDeviceKind::MusicDreamView
                        && !has_saved_control_id(entry)
                        && definition.member_ids.len() <= 1;
                    if !empty_music_placeholder {
                        definitions.push(definition);
                    }
                }
            }
        }
    }

    definitions
}

/// IDs of the user-selected video DreamView sync centers. A generic
/// `dreamViewToggle` capability is also advertised on Music DreamView centers,
/// so capability metadata alone is not sufficient to expose the video switch.
pub fn parse_video_dreamview_centers(components: &[OneClickComponent]) -> HashSet<String> {
    let mut centers = HashSet::new();
    for component in components {
        if component.component_type != 2 || component.feast_type != Some(3) {
            continue;
        }
        if let Some(main) = component.main_device.as_ref().and_then(device_id) {
            centers.insert(main.to_ascii_lowercase());
        }
        for entry in component
            .feasts
            .iter()
            .chain(component.environments.iter())
        {
            if let Some(main) = entry.get("feastMainDevice").and_then(device_id) {
                centers.insert(main.to_ascii_lowercase());
            }
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

/// Aggregate a virtual control from its physical members. The control is ON
/// only when every member has a known ON state. A known OFF member is enough
/// to report OFF; otherwise incomplete membership/state remains unknown rather
/// than being rendered as a false OFF value.
pub fn aggregate_virtual_state(
    definition: &VirtualControlDefinition,
    devices: &HashMap<String, Device>,
) -> Option<DeviceState> {
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
    fn parses_base_group_and_dreamview_members_without_names_hardcoded() {
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

        let dreamview = definitions
            .iter()
            .find(|definition| definition.kind == VirtualDeviceKind::DreamViewScene)
            .unwrap();
        assert_eq!(dreamview.name, "Scenic DreamView");
        assert!(dreamview.member_ids.iter().any(|id| id == ":11"));
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
            kind: VirtualDeviceKind::DreamViewScene,
            ids: vec![],
            name: "Scene".into(),
            member_ids: vec!["one".into(), "missing".into()],
            control_device_id: None,
        };
        let mut devices = HashMap::new();
        devices.insert("one".into(), device("one", true));
        assert!(aggregate_virtual_state(&definition, &devices).is_none());
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
                "name": "MusicLight-1",
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
        assert_eq!(music.name, "MusicLight-1");
        assert_eq!(music.control_device_id.as_deref(), Some("CENTER"));
        assert_eq!(music.member_ids, vec!["CENTER", "MEMBER-1", "MEMBER-2"]);
    }

    #[test]
    fn video_dreamview_selects_center_without_creating_virtual_scene() {
        let components = vec![dreamview_component(
            3,
            serde_json::json!({"device": "VIDEO-CENTER"}),
            vec![],
        )];

        assert!(parse_virtual_controls(&components).is_empty());
        assert!(parse_video_dreamview_centers(&components).contains("video-center"));
    }

    #[test]
    fn empty_music_dreamview_placeholder_is_not_exposed() {
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
