#!/bin/bash

# Build NS8-SoftwareManager application and NS8 module images
set -euo pipefail

REPOBASE="${REPOBASE:-ghcr.io/${GITHUB_REPOSITORY_OWNER:-bls-isp}}"
REPOBASE="$(printf '%s' "${REPOBASE}" | tr '[:upper:]' '[:lower:]')"
IMAGETAG="${IMAGETAG:-latest}"

if command -v podman >/dev/null 2>&1; then
    CONTAINER_ENGINE="${CONTAINER_ENGINE:-podman}"
else
    CONTAINER_ENGINE="${CONTAINER_ENGINE:-docker}"
fi

APP_IMAGE="${REPOBASE}/softwaremanager-app:${IMAGETAG}"
MODULE_IMAGE="${REPOBASE}/softwaremanager:${IMAGETAG}"

echo "==> Building application container: ${APP_IMAGE}"
"${CONTAINER_ENGINE}" build \
    -f Containerfile.app \
    -t "${APP_IMAGE}" \
    .

echo "==> Building NS8 module container: ${MODULE_IMAGE}"
"${CONTAINER_ENGINE}" build \
    -f Containerfile \
    --build-arg "APP_IMAGE=${APP_IMAGE}" \
    -t "${MODULE_IMAGE}" \
    .

images=("${APP_IMAGE}" "${MODULE_IMAGE}")

if [[ "${TAG_LATEST:-0}" == "1" && "${IMAGETAG}" != "latest" ]]; then
    APP_LATEST="${REPOBASE}/softwaremanager-app:latest"
    MODULE_LATEST="${REPOBASE}/softwaremanager:latest"

    echo "==> Tagging latest application container: ${APP_LATEST}"
    "${CONTAINER_ENGINE}" tag "${APP_IMAGE}" "${APP_LATEST}"

    echo "==> Building latest NS8 module container: ${MODULE_LATEST}"
    "${CONTAINER_ENGINE}" build \
        -f Containerfile \
        --build-arg "APP_IMAGE=${APP_LATEST}" \
        -t "${MODULE_LATEST}" \
        .

    images+=("${APP_LATEST}" "${MODULE_LATEST}")
fi

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf "images=%s\n" "${images[*]}" >> "${GITHUB_OUTPUT}"
else
    printf "Built images:\n"
    for img in "${images[@]}"; do
        printf "  - %s\n" "${img}"
    done
fi

if [[ "${PUSH:-0}" == "1" ]]; then
    for img in "${images[@]}"; do
        echo "==> Pushing ${img}"
        "${CONTAINER_ENGINE}" push "${img}"
    done
fi
