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
}

impl VirtualDeviceKind {
    pub fn from_sku(sku: &str) -> Option<Self> {
        match sku {
            "BaseGroup" => Some(Self::BaseGroup),
            "DreamViewScenic" => Some(Self::DreamViewScene),
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

fn append_device_id(member_ids: &mut Vec<String>, device: &JsonValue) {
    let value = device.get("device").unwrap_or(device);
    if let Some(id) = json_identifier(Some(value)) {
        push_unique_case_insensitive(member_ids, id);
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

    let mut member_ids = vec![];
    if let Some(devices) = object.get("devices").and_then(JsonValue::as_array) {
        for device in devices {
            append_device_id(&mut member_ids, device);
        }
    }
    if let Some(main) = object.get("feastMainDevice") {
        append_device_id(&mut member_ids, main);
    }
    if let Some(main) = component_main_device {
        append_device_id(&mut member_ids, main);
    }

    (!name.is_empty() || !ids.is_empty() || !member_ids.is_empty()).then_some(
        VirtualControlDefinition {
            kind,
            ids,
            name,
            member_ids,
        },
    )
}

/// Extract BaseGroup and scenic DreamView membership from the undocumented
/// home-layout response. Matching later uses IDs first, then the display name,
/// and finally a unique object of the same kind. This handles API generations
/// that expose `gId`, `groupId`, or `feastId` for the same Platform object.
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
            for entry in &dreamview_entries {
                if let Some(definition) = definition_from_json(
                    entry,
                    VirtualDeviceKind::DreamViewScene,
                    Some(component.component_id),
                    component.main_device.as_ref(),
                    dreamview_entries.len() == 1,
                ) {
                    definitions.push(definition);
                }
            }
        }
    }

    definitions
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
        };
        let mut devices = HashMap::new();
        devices.insert("one".into(), device("one", true));
        assert!(aggregate_virtual_state(&definition, &devices).is_none());
    }
}
