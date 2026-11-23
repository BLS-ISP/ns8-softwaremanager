# ==============================================================================
# Stage 1: Build Vue 3 Admin Web Console & Software Center Desktop UI Assets
# ==============================================================================
FROM docker.io/library/node:20-alpine AS web-builder

WORKDIR /web
COPY web/package*.json ./
RUN npm install
COPY web/ ./
RUN npm run build

WORKDIR /center-ui
COPY agent/crates/swm-center/ui/package*.json ./
RUN npm install
COPY agent/crates/swm-center/ui/ ./
RUN npm run build

# ==============================================================================
# Stage 2: Build Linux Rust Workspace Binaries (swm-agent & swm-center)
#   Copies /center-ui/dist BEFORE cargo build so swm-center embeds fresh UI!
# ==============================================================================
FROM docker.io/library/rust:1-bookworm AS agent-builder

WORKDIR /src/agent
COPY agent/ ./
COPY prebuilt-agents/linux/ /prebuilt-linux/
RUN rm -rf /src/agent/crates/swm-center/ui/dist
COPY --from=web-builder /center-ui/dist /src/agent/crates/swm-center/ui/dist

RUN mkdir -p /out/linux /out/linux-amd64 /out/windows && \
    if [ -f /prebuilt-linux/swm-agent ] && [ -f /prebuilt-linux/swm-center ]; then \
        cp /prebuilt-linux/swm-agent /prebuilt-linux/swm-center /out/linux/ && \
        cp /prebuilt-linux/swm-agent /prebuilt-linux/swm-center /out/linux-amd64/ && \
        chmod 0755 /out/linux/swm-agent /out/linux/swm-center /out/linux-amd64/swm-agent /out/linux-amd64/swm-center; \
    else \
        cargo build --release --workspace && \
        cp target/release/swm-agent target/release/swm-center /out/linux/ && \
        cp target/release/swm-agent target/release/swm-center /out/linux-amd64/; \
    fi && \
    cp -r /src/agent/crates/swm-center/ui/dist /out/linux/swm-center-ui && \
    cp -r /src/agent/crates/swm-center/ui/dist /out/linux-amd64/swm-center-ui

# ==============================================================================
# Stage 3: FastAPI Management Server Runtime Image
# ==============================================================================
FROM docker.io/library/python:3.12-slim-bookworm

LABEL org.opencontainers.image.source="https://github.com/BLS-ISP/ns8-softwaremanager" \
      org.opencontainers.image.description="FastAPI Server, Admin Web Console & Client Binaries for NS8-SoftwareManager"

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    DATA_DIR=/data \
    WEB_DIST_DIR=/app/web/dist \
    STATIC_AGENTS_DIR=/app/agents

WORKDIR /app/server

COPY server/requirements.txt /app/server/requirements.txt
RUN pip install --no-cache-dir -r /app/server/requirements.txt

COPY server/ /app/server/
COPY --from=web-builder /web/dist /app/web/dist
COPY --from=agent-builder /out/ /app/agents/
COPY prebuilt-agents/windows/ /app/agents/windows/

RUN useradd --create-home --uid 1001 swm && \
    mkdir -p /data /app/agents/linux /app/agents/linux-amd64 /app/agents/windows && \
    chown -R swm:swm /app /data

USER swm
EXPOSE 8080

CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8080"]
