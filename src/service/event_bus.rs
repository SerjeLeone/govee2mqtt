use tokio::sync::broadcast;

/// Events that flow through the system.
/// Extensions and other components can subscribe to these
/// instead of calling State methods directly.
#[derive(Clone, Debug)]
pub enum Event {
    /// A device's state changed (e.g., new poll data, control command completed).
    DeviceStateChanged { device_id: String },

    /// A new device was discovered via any API.
    DeviceDiscovered { device_id: String, sku: String },

    /// The device config file was reloaded.
    ConfigReloaded,

    /// The bridge is shutting down.
    Shutdown,
}

pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    /// Publish an event to all subscribers.
    pub fn emit(&self, event: Event) {
        // Ignore send errors (no subscribers is fine)
        let _ = self.tx.send(event);
    }

    /// Subscribe to the event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_with_no_subscribers_does_not_panic() {
        let bus = EventBus::new();
        bus.emit(Event::ConfigReloaded);
        bus.emit(Event::Shutdown);
        bus.emit(Event::DeviceStateChanged {
            device_id: "test".into(),
        });
    }

    #[tokio::test]
    async fn subscriber_receives_emitted_events() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.emit(Event::ConfigReloaded);

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, Event::ConfigReloaded));
    }

    #[tokio::test]
    async fn multiple_subscribers_each_get_the_event() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.emit(Event::DeviceDiscovered {
            device_id: "dev1".into(),
            sku: "H6001".into(),
        });

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();

        match (&e1, &e2) {
            (
                Event::DeviceDiscovered {
                    device_id: id1,
                    sku: s1,
                },
                Event::DeviceDiscovered {
                    device_id: id2,
                    sku: s2,
                },
            ) => {
                assert_eq!(id1, "dev1");
                assert_eq!(s1, "H6001");
                assert_eq!(id2, "dev1");
                assert_eq!(s2, "H6001");
            }
            _ => panic!("Expected DeviceDiscovered events"),
        }
    }

    #[tokio::test]
    async fn subscriber_receives_multiple_events_in_order() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.emit(Event::ConfigReloaded);
        bus.emit(Event::Shutdown);

        let e1 = rx.recv().await.unwrap();
        let e2 = rx.recv().await.unwrap();
        assert!(matches!(e1, Event::ConfigReloaded));
        assert!(matches!(e2, Event::Shutdown));
    }
}
