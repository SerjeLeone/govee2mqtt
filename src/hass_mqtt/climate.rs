use crate::hass_mqtt::base::{Device, EntityConfig, Origin};
use crate::hass_mqtt::instance::{lookup_entity_device, EntityInstance};
use crate::hass_mqtt::number::NumberConfig;
use crate::platform_api::{DeviceCapability, DeviceParameters};
use crate::service::device::Device as ServiceDevice;
use crate::service::hass::{
    device_availability_entries, topic_safe_id, topic_safe_string, HassClient,
};
use crate::service::state::StateHandle;
use crate::temperature::{
    TemperatureScale, TemperatureUnits, TemperatureValue, DEVICE_CLASS_TEMPERATURE,
};
use anyhow::anyhow;
use mosquitto_rs::router::{Params, Payload, State};
use serde::Deserialize;
use std::str::FromStr;

// TODO: register an actual climate entity.
// I don't have one of these devices, so it is currently guesswork!

pub struct TargetTemperatureEntity {
    number: NumberConfig,
    device_id: String,
    state: StateHandle,
    instance_name: String,
}

pub struct TemperatureConstraints {
    pub min: TemperatureValue,
    pub max: TemperatureValue,
}

impl TemperatureConstraints {
    pub fn as_unit(&self, unit: TemperatureUnits) -> Self {
        Self {
            min: self.min.as_unit(unit),
            max: self.max.as_unit(unit),
        }
    }
}

pub fn parse_temperature_constraints(
    instance: &DeviceCapability,
) -> anyhow::Result<TemperatureConstraints> {
    let units = instance
        .struct_field_by_name("unit")
        .and_then(|field| {
            field.default_value.as_ref().and_then(|v| {
                v.as_str()
                    .and_then(|s| TemperatureScale::from_str(s).map(Into::into).ok())
            })
        })
        .unwrap_or(TemperatureUnits::Fahrenheit);

    let temperature = instance
        .struct_field_by_name("temperature")
        .ok_or_else(|| anyhow!("no temperature field in {instance:?}"))?;
    match &temperature.field_type {
        DeviceParameters::Integer { unit, range } => {
            let range_units = unit
                .as_deref()
                .and_then(|s| TemperatureScale::from_str(s).map(Into::into).ok())
                .unwrap_or(units);

            let min = TemperatureValue::new(range.min.into(), range_units);
            let max = TemperatureValue::new(range.max.into(), range_units);

            Ok(TemperatureConstraints {
                min: min.as_unit(units),
                max: max.as_unit(units),
            })
        }
        _ => {
            anyhow::bail!("Unexpected temperature value in {instance:?}");
        }
    }
}

impl TargetTemperatureEntity {
    pub async fn new(
        device: &ServiceDevice,
        state: &StateHandle,
        instance: &DeviceCapability,
    ) -> anyhow::Result<Self> {
        let units = state.get_temperature_scale().await;

        let constraints = parse_temperature_constraints(instance)?.as_unit(units.into());
        let unique_id = format!(
            "{id}-{inst}",
            id = topic_safe_id(device),
            inst = topic_safe_string(&instance.instance)
        );

        let name = "Target Temperature".to_string();
        let command_topic = format!(
            "gv2mqtt/{id}/set-temperature/{inst}/{units}",
            id = topic_safe_id(device),
            inst = topic_safe_string(&instance.instance)
        );
        let state_topic = format!(
            "gv2mqtt/{id}/advise-set-temperature",
            id = topic_safe_id(device),
        );

        let (availability, availability_mode) = device_availability_entries(device);

        Ok(Self {
            number: NumberConfig {
                base: EntityConfig {
                    availability_topic: String::new(),
                    availability,
                    availability_mode,
                    name: Some(name),
                    entity_category: None,
                    origin: Origin::default(),
                    device: Device::for_device(device),
                    unique_id: unique_id.clone(),
                    device_class: Some(DEVICE_CLASS_TEMPERATURE),
                    icon: Some("mdi:thermometer".to_string()),
                },
                state_topic: Some(state_topic),
                command_topic,
                min: Some(constraints.min.value().floor() as f32),
                max: Some(constraints.max.value().ceil() as f32),
                step: 1.0,
                unit_of_measurement: Some(units.unit_of_measurement()),
                enabled_by_default: None,
            },
            device_id: device.id.to_string(),
            state: state.clone(),
            instance_name: instance.instance.to_string(),
        })
    }
}

#[async_trait::async_trait]
impl EntityInstance for TargetTemperatureEntity {
    async fn publish_config(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()> {
        self.number.publish(&state, &client).await
    }

    async fn notify_state(&self, client: &HassClient) -> anyhow::Result<()> {
        let Some(device) =
            lookup_entity_device(&self.state, &self.device_id, "target temperature entity").await
        else {
            return Ok(());
        };

        let quirk = device.resolve_quirk();

        log::debug!("notify_state for {device} {}", self.instance_name);

        if let Some(cap) = device.get_state_capability_by_instance(&self.instance_name) {
            log::debug!("have: {cap:?}");

            let units = cap
                .state
                .pointer("/value/unit")
                .and_then(|unit| {
                    unit.as_str()
                        .and_then(|s| TemperatureScale::from_str(s).map(Into::into).ok())
                })
                .or_else(|| quirk.and_then(|q| q.platform_temperature_sensor_units))
                .unwrap_or(TemperatureUnits::Celsius);

            log::debug!("units are reported as {units:?}");

            let value = match cap
                .state
                .pointer("/value/targetTemperature")
                .and_then(|v| v.as_f64())
                .map(|v| TemperatureValue::new(v, units))
            {
                Some(v) => {
                    let pref_units = self.state.get_temperature_scale().await;
                    log::debug!("reported temp is {v}, pref_units: {pref_units}");
                    let value = v.as_unit(pref_units.into()).value();
                    format!("{value:.2}")
                }
                None => "".to_string(),
            };

            log::debug!("setting value to {value}");

            return self.number.notify_state(&client, &value).await;
        }

        Ok(())
    }
}

#[derive(Deserialize)]
pub struct IdInstAndUnits {
    id: String,
    instance: String,
    units: String,
}

pub async fn mqtt_set_temperature(
    Payload(value): Payload<String>,
    Params(IdInstAndUnits {
        id,
        instance,
        units,
    }): Params<IdInstAndUnits>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    log::info!("Command: set-temperature for {id}: {value}");
    let device = state.resolve_device_for_control(&id).await?;

    let scale: TemperatureScale = units.parse()?;
    let target_value = TemperatureValue::parse_with_optional_scale(&value, Some(scale))?;

    state
        .device_set_target_temperature(&device, &instance, target_value)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TargetTemperatureEntity;
    use crate::platform_api::{
        DeviceCapability, DeviceCapabilityKind, DeviceParameters, IntegerRange, StructField,
    };
    use crate::service::device::Device;
    use crate::service::state::State;
    use crate::temperature::TemperatureScale;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn target_temperature_entity_keeps_enabled_by_default_unset() {
        let device = Device::new("H7160", "AA:BB");
        let state = Arc::new(State::new());
        state.set_temperature_scale(TemperatureScale::Celsius).await;

        let capability = DeviceCapability {
            kind: DeviceCapabilityKind::TemperatureSetting,
            instance: "targetTemperature".to_string(),
            parameters: Some(DeviceParameters::Struct {
                fields: vec![
                    StructField {
                        field_name: "unit".to_string(),
                        field_type: DeviceParameters::Enum { options: vec![] },
                        default_value: Some(json!("C")),
                        required: true,
                    },
                    StructField {
                        field_name: "temperature".to_string(),
                        field_type: DeviceParameters::Integer {
                            unit: Some("C".to_string()),
                            range: IntegerRange {
                                min: 20,
                                max: 90,
                                precision: 1,
                            },
                        },
                        default_value: None,
                        required: true,
                    },
                ],
            }),
            alarm_type: None,
            event_state: None,
        };

        let entity = TargetTemperatureEntity::new(&device, &state, &capability)
            .await
            .unwrap();

        assert_eq!(entity.number.enabled_by_default, None);
        assert_eq!(entity.number.unit_of_measurement, Some("°C"));
    }
}
