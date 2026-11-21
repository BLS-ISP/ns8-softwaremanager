# ==============================================================================
# Stage 1: Build NS8 Cluster-Admin Cockpit UI (Vue 2 + @nethserver/ns8-ui-lib)
# ==============================================================================
FROM docker.io/library/node:20-alpine AS ui-builder

WORKDIR /ui
COPY ui/package*.json ./
RUN npm install --legacy-peer-deps
COPY ui/ ./
RUN npm run build

# ==============================================================================
# Stage 2: Prepare NS8 imageroot (strip CRLF, clean bytecode, set permissions)
# ==============================================================================
FROM docker.io/library/alpine:3.20 AS imageroot-prep

COPY imageroot /imageroot
RUN find /imageroot -name "__pycache__" -type d -exec rm -rf {} + && \
    find /imageroot -name "*.pyc" -delete && \
    find /imageroot -type f -exec sed -i 's/\r$//' {} + && \
    chmod +x /imageroot/actions/*/[0-9]* /imageroot/update-module.d/* /imageroot/bin/*

# ==============================================================================
# Stage 3: Final Rootless NethServer 8 Module Image
# ==============================================================================
FROM scratch

ARG APP_IMAGE="ghcr.io/bls-isp/softwaremanager-app:latest"

COPY --from=imageroot-prep /imageroot /imageroot
COPY --from=ui-builder /ui/dist /ui

LABEL org.opencontainers.image.source="https://github.com/BLS-ISP/ns8-softwaremanager" \
      org.opencontainers.image.description="NethServer 8 Module for NS8-SoftwareManager" \
      org.nethserver.name="softwaremanager" \
      org.nethserver.description="Central Endpoint, OS Patch, and Self-Service Software Management for Windows & Linux" \
      org.nethserver.authorizations="traefik@node:routeadm cluster:accountconsumer" \
      org.nethserver.tcp-ports-demand="1" \
      org.nethserver.rootfull="0" \
      org.nethserver.images="docker.io/library/postgres:16-alpine ${APP_IMAGE}"
