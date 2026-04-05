ARG BUILD_FROM
ARG GOVEE2MQTT_IMAGE=ghcr.io/serjeleone/govee2mqtt:latest
FROM ${GOVEE2MQTT_IMAGE} AS govee2mqtt
FROM $BUILD_FROM
COPY run.sh /run.sh
COPY --from=govee2mqtt /app/govee /app/govee
COPY --from=govee2mqtt /app/assets /app/assets/
COPY --from=govee2mqtt /app/AmazonRootCA1.pem /app/AmazonRootCA1.pem
COPY --from=govee2mqtt /app/scene-data /app/scene-data/

HEALTHCHECK --interval=1m --timeout=3s --start-period=30s --start-interval=5s \
  CMD curl -sf http://localhost:8056/api/health || exit 1

CMD [ "/run.sh" ]
