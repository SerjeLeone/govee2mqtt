ARG BUILD_FROM
ARG GOVEE2MQTT_IMAGE=ghcr.io/wez/govee2mqtt:latest
FROM ${GOVEE2MQTT_IMAGE} AS govee2mqtt
FROM $BUILD_FROM
COPY run.sh /run.sh
COPY --from=govee2mqtt /app/govee /app/govee
COPY --from=govee2mqtt /app/assets /app/assets/
COPY --from=govee2mqtt /app/AmazonRootCA1.pem /app/AmazonRootCA1.pem
CMD [ "/run.sh" ]
