use crate::hass_mqtt::base::EntityConfig;
use crate::service::device::Device as ServiceDevice;
use crate::service::hass::HassClient;
use crate::service::state::StateHandle;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

#[async_trait]
pub trait EntityInstance: Send + Sync {
    async fn publish_config(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()>;
    async fn notify_state(&self, client: &HassClient) -> anyhow::Result<()>;
}

pub async fn publish_entity_config<T: Serialize>(
    integration: &str,
    state: &StateHandle,
    client: &HassClient,
    base: &EntityConfig,
    config: &T,
) -> anyhow::Result<()> {
    // TODO: remember all published topics for future GC

    let disco = state.get_hass_disco_prefix().await;
    let topic = format!(
        "{disco}/{integration}/{unique_id}/config",
        unique_id = base.unique_id
    );

    client.publish_obj_retained(topic, config).await
}

pub async fn lookup_entity_device(
    state: &StateHandle,
    device_id: &str,
    entity: &str,
) -> Option<ServiceDevice> {
    let device = state.device_by_id(device_id).await;
    if device.is_none() {
        log::warn!("Skipping {entity} notify_state for missing device {device_id}");
    }
    device
}

#[derive(Default, Clone)]
pub struct EntityList {
    entities: Vec<Arc<dyn EntityInstance + Send + Sync + 'static>>,
}

impl EntityList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<E: EntityInstance + Send + Sync + 'static>(&mut self, e: E) {
        self.entities.push(Arc::new(e));
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub async fn publish_config(
        &self,
        state: &StateHandle,
        client: &HassClient,
    ) -> anyhow::Result<()> {
        // Allow HASS time to process each entity before registering the next
        let delay = tokio::time::Duration::from_millis(100);
        for e in &self.entities {
            if let Err(err) = e.publish_config(state, client).await {
                log::warn!("EntityList::publish_config: {err:#}");
            }
            tokio::time::sleep(delay).await;
        }
        Ok(())
    }

    pub async fn notify_state(&self, client: &HassClient) -> anyhow::Result<()> {
        for e in &self.entities {
            if let Err(err) = e.notify_state(client).await {
                log::warn!("EntityList::notify_state: {err:#}");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::EntityList;
    use crate::hass_mqtt::number::MusicSensitivityNumber;
    use crate::hass_mqtt::sensor::GlobalFixedDiagnostic;
    use crate::service::device::Device;
    use crate::service::hass::HassClient;
    use crate::service::state::State;
    use std::sync::Arc;

    #[tokio::test]
    async fn entity_list_skips_missing_devices_and_continues_notifying() {
        let state = Arc::new(State::new());
        let missing_device = Device::new("H6000", "AA:BB");
        let missing_entity = MusicSensitivityNumber::new(&missing_device, &state);
        let healthy_entity = GlobalFixedDiagnostic::new("Version", "1.2.3");
        let client = HassClient::new_test();

        let mut entities = EntityList::new();
        entities.add(missing_entity);
        entities.add(healthy_entity);

        entities.notify_state(&client).await.unwrap();

        assert_eq!(
            client.published_messages(),
            vec![(
                "gv2mqtt/sensor/global-version/state".to_string(),
                "1.2.3".to_string()
            )]
        );
    }
}
