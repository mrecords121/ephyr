#!/usr/bin/env bash

set -e

# Install Podman for running containers.
echo "deb https://download.opensuse.org/repositories/devel:/kubic:/libcontainers:/stable/xUbuntu_20.04/ /" \
  | tee /etc/apt/sources.list.d/devel:kubic:libcontainers:stable.list
curl -L https://download.opensuse.org/repositories/devel:/kubic:/libcontainers:/stable/xUbuntu_20.04/Release.key \
  | apt-key add -
apt-get -y update
apt-get -y install podman

# Install Ephyr re-streamer.
cat << 'EOF' > /etc/systemd/system/ephyr-restreamer.service
[Unit]
Description=Ephyr service for re-streaming RTMP streams
After=podman.service


[Service]
Environment=EPHYR_CONTAINER_NAME=ephyr-restreamer
Environment=EPHYR_IMAGE_NAME=docker.io/allatra/ephyr
Environment=EPHYR_IMAGE_TAG=restreamer-0.1

ExecStartPre=/usr/bin/mkdir -p /var/lib/${EPHYR_CONTAINER_NAME}/
ExecStartPre=touch /var/lib/${EPHYR_CONTAINER_NAME}/srs.conf
ExecStartPre=touch /var/lib/${EPHYR_CONTAINER_NAME}/state.json

ExecStartPre=-/usr/bin/podman pull ${EPHYR_IMAGE_NAME}:${EPHYR_IMAGE_TAG}
ExecStartPre=-/usr/bin/podman stop ${EPHYR_CONTAINER_NAME}
ExecStartPre=-/usr/bin/podman rm --volumes ${EPHYR_CONTAINER_NAME}
ExecStart=/usr/bin/podman run \
  --network=host \
  -v /var/lib/${EPHYR_CONTAINER_NAME}/srs.conf:/usr/local/srs/conf/srs.conf \
  -v /var/lib/${EPHYR_CONTAINER_NAME}/state.json:/state.json \
  --name=${EPHYR_CONTAINER_NAME} ${EPHYR_IMAGE_NAME}:${EPHYR_IMAGE_TAG}

ExecStop=-/usr/bin/podman stop ${EPHYR_CONTAINER_NAME}
ExecStop=-/usr/bin/podman rm --volumes ${EPHYR_CONTAINER_NAME}

Restart=always


[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl unmask ephyr-restreamer.service
systemctl enable ephyr-restreamer.service
systemctl start ephyr-restreamer.service
