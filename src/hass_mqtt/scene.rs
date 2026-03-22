use crate::hass_mqtt::base::EntityConfig;
use crate::hass_mqtt::instance::{publish_entity_config, EntityInstance};
use crate::service::hass::HassClient;
use crate::service::state::StateHandle;
use async_trait::async_trait;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct SceneConfig {
    #[serde(flatten)]
    pub base: EntityConfig,

    pub command_topic: String,
    pub payload_on: String,
}

impl SceneConfig {
    pub async fn publish(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()> {
        publish_entity_config("scene", state, client, &self.base, self).await
    }
}

#[async_trait]
impl EntityInstance for SceneConfig {
    async fn publish_config(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()> {
        self.publish(&state, &client).await
    }

    async fn notify_state(&self, _client: &HassClient) -> anyhow::Result<()> {
        // Scenes have no state
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SceneConfig;
    use crate::hass_mqtt::base::{Device, EntityConfig, Origin};
    use crate::hass_mqtt::instance::EntityInstance;
    use crate::service::hass::{availability_topic, HassClient};
    use crate::service::state::State;
    use std::sync::Arc;

    #[tokio::test]
    async fn scene_config_publishes_config_without_state_without_broker() {
        let state = Arc::new(State::new());
        state
            .set_hass_disco_prefix("homeassistant".to_string())
            .await;
        let client = HassClient::new_test();
        let scene = SceneConfig {
            base: EntityConfig {
                availability_topic: availability_topic(),
                name: Some("Movie Time".to_string()),
                device_class: None,
                origin: Origin::default(),
                device: Device::this_service(),
                unique_id: "scene-movie-time".to_string(),
                entity_category: None,
                icon: None,
            },
            command_topic: "gv2mqtt/oneclick".to_string(),
            payload_on: "Movie Time".to_string(),
        };

        scene.publish_config(&state, &client).await.unwrap();
        scene.notify_state(&client).await.unwrap();

        let published = client.published_messages();
        assert_eq!(published.len(), 1);
        assert_eq!(
            published[0].0,
            "homeassistant/scene/scene-movie-time/config"
        );
        assert!(published[0]
            .1
            .contains("\"command_topic\":\"gv2mqtt/oneclick\""));
        assert!(published[0].1.contains("\"payload_on\":\"Movie Time\""));
    }
}
