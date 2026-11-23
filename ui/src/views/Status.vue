<template>
  <div class="ns-page-content">
    <!-- Top Page Header & Primary Actions -->
    <div class="page-toolbar">
      <div class="status-brand">
        <img
          class="status-logo"
          src="assets/module_default_logo.svg"
          alt="SoftwareManager"
        />
        <div>
          <h2 class="page-title">{{ $t("status.title") }}</h2>
          <p class="page-subtitle">{{ $t("status.subtitle") }}</p>
        </div>
      </div>

      <div class="toolbar-actions" v-if="config && config.app_url">
        <NsButton
          kind="tertiary"
          :icon="Laptop20"
          @click="openEnrollmentWizard"
        >
          {{ $t("status.open_enrollment") }}
        </NsButton>
        <NsButton
          kind="primary"
          :icon="Launch20"
          @click="openAdminConsole"
        >
          {{ $t("status.open_webapp") }}
        </NsButton>
      </div>
    </div>

    <!-- Unconfigured Banner -->
    <div v-if="!loadingConfig && (!config || !config.configured)" class="mg-bottom-lg">
      <NsInlineNotification
        kind="warning"
        :title="$t('status.not_configured_title')"
        :description="$t('status.not_configured_description')"
        :actionLabel="$t('status.go_to_settings')"
        @action="goToSettings"
        :showCloseButton="false"
      />
    </div>

    <!-- 4-Card Instance & Routing Overview -->
    <div class="card-grid">
      <!-- 1. App Instance Card -->
      <NsInfoCard
        light
        :wrapTitle="true"
        :title="instanceLabel || instanceName || 'softwaremanager'"
        :description="$t('status.app_instance')"
        :icon="Application32"
        :loading="loadingStatus"
        class="ns-card"
      >
        <template slot="content">
          <div class="card-slot-body">
            <div class="kv-list">
              <div class="kv-row">
                <span class="kv-label">{{ $t("status.installation_node") }}</span>
                <span class="kv-value">
                  {{
                    status && status.node
                      ? $t("status.node", { node: status.node })
                      : "-"
                  }}
                </span>
              </div>
              <div class="kv-row" v-if="config && config.tcp_port">
                <span class="kv-label">{{ $t("status.internal_port") }}</span>
                <span class="kv-value kv-mono">127.0.0.1:{{ config.tcp_port }}</span>
              </div>
            </div>
            <div class="card-cta">
              <NsButton
                v-if="config && config.app_url"
                kind="ghost"
                :icon="Launch20"
                @click="openAdminConsole"
              >
                {{ $t("status.open_webapp") }}
              </NsButton>
              <NsButton
                v-else
                kind="ghost"
                :icon="Settings20"
                @click="goToSettings"
              >
                {{ $t("status.go_to_settings") }}
              </NsButton>
            </div>
          </div>
        </template>
      </NsInfoCard>

      <!-- 2. Endpoint & Portal Routing Card (always-mounted slot wrapper fixes Vue 2 $slots caching) -->
      <NsInfoCard
        light
        :wrapTitle="true"
        :title="$t('status.configuration_summary')"
        :description="config && config.domain_name ? config.domain_name : '-'"
        :icon="EdgeNode32"
        :loading="loadingConfig"
        class="ns-card"
      >
        <template slot="content">
          <div class="card-slot-body">
            <div v-if="config" class="kv-list">
              <div class="kv-row">
                <span class="kv-label">{{ $t("status.lets_encrypt") }}</span>
                <span class="kv-value">
                  <span :class="['pill-badge', config.lets_encrypt ? 'pill-green' : 'pill-gray']">
                    {{ config.lets_encrypt ? $t("common.enabled") : $t("common.disabled") }}
                  </span>
                </span>
              </div>
              <div class="kv-row">
                <span class="kv-label">{{ $t("status.http2https") }}</span>
                <span class="kv-value">
                  <span :class="['pill-badge', config.http2https ? 'pill-green' : 'pill-gray']">
                    {{ config.http2https ? $t("common.enabled") : $t("common.disabled") }}
                  </span>
                </span>
              </div>
              <div class="kv-row">
                <span class="kv-label">{{ $t("status.ldap_domain") }}</span>
                <span class="kv-value">
                  <span v-if="config.ldap_domain" class="pill-badge pill-blue">{{ config.ldap_domain }}</span>
                  <span v-else>{{ $t("status.local_only") }}</span>
                </span>
              </div>
              <div class="kv-row">
                <span class="kv-label">{{ $t("status.admin_username") }}</span>
                <span class="kv-value kv-mono">{{ config.admin_username || "admin" }}</span>
              </div>
            </div>
          </div>
        </template>
      </NsInfoCard>

      <!-- 3. Backup Card -->
      <NsBackupCard
        :title="core ? core.$t('backup.title') : 'Backup'"
        :noBackupMessage="core ? core.$t('backup.no_backup_configured') : 'No backup configured'"
        :goToBackupLabel="core ? core.$t('backup.go_to_backup') : 'Go to Backup'"
        :repositoryLabel="core ? core.$t('backup.repository') : 'Repository'"
        :statusLabel="core ? core.$t('common.status') : 'Status'"
        :statusSuccessLabel="core ? core.$t('common.success') : 'Success'"
        :statusNotRunLabel="core ? core.$t('backup.backup_has_not_run_yet') : 'Not run yet'"
        :statusErrorLabel="core ? core.$t('error.error') : 'Error'"
        :completedLabel="core ? core.$t('backup.completed') : 'Completed'"
        :durationLabel="core ? core.$t('backup.duration') : 'Duration'"
        :totalSizeLabel="core ? core.$t('backup.total_size') : 'Total size'"
        :totalFileCountLabel="core ? core.$t('backup.total_file_count') : 'Total files'"
        :backupDisabledLabel="core ? core.$t('common.disabled') : $t('common.disabled')"
        :showOnlyMoreInfo="false"
        :moduleId="instanceName"
        :moduleUiName="instanceLabel"
        :repositories="backupRepositories"
        :backups="backups"
        :loading="loadingBackups"
        :coreContext="core"
        light
      />

      <!-- 4. System Logs Card -->
      <NsSystemLogsCard
        :title="core ? core.$t('system_logs.title') : 'System logs'"
        :description="
          core
            ? core.$t('system_logs.card_description', { name: instanceLabel || instanceName })
            : instanceName
        "
        :buttonLabel="core ? core.$t('system_logs.card_button_label') : 'Open logs'"
        :router="core ? core.$router : null"
        context="module"
        :moduleId="instanceName"
        light
      />
    </div>

    <!-- Client Onboarding & Agent Deployment Guide (shown once configured) -->
    <cv-tile v-if="config && config.configured && config.app_url" light class="onboarding-tile mg-bottom-lg">
      <div class="onboarding-header">
        <div>
          <h3 class="onboarding-title">{{ $t("status.onboarding_title") }}</h3>
          <p class="onboarding-subtitle">
            {{ $t("status.onboarding_subtitle", { url: config.app_url }) }}
          </p>
        </div>
        <NsButton
          kind="primary"
          :icon="ArrowRight20"
          @click="openEnrollmentWizard"
        >
          {{ $t("status.open_rollout_wizard") }}
        </NsButton>
      </div>

      <div class="onboarding-steps">
        <div class="step-card">
          <div class="step-badge">1</div>
          <div>
            <h4 class="step-heading">{{ $t("status.step1_title") }}</h4>
            <p class="step-desc">{{ $t("status.step1_desc") }}</p>
          </div>
        </div>

        <div class="step-card">
          <div class="step-badge">2</div>
          <div>
            <h4 class="step-heading">{{ $t("status.step2_title") }}</h4>
            <p class="step-desc">{{ $t("status.step2_desc") }}</p>
          </div>
        </div>

        <div class="step-card">
          <div class="step-badge">3</div>
          <div>
            <h4 class="step-heading">{{ $t("status.step3_title") }}</h4>
            <p class="step-desc">{{ $t("status.step3_desc") }}</p>
          </div>
        </div>
      </div>

      <div class="onboarding-footer">
        <div class="endpoint-copy-box">
          <span class="endpoint-label">{{ $t("status.server_endpoint_label") }}</span>
          <code class="endpoint-code">{{ config.app_url }}</code>
          <button
            type="button"
            class="copy-inline-btn"
            @click="copyServerUrl"
          >
            <Checkmark16 v-if="copiedUrl" />
            <Copy16 v-else />
            <span>{{ copiedUrl ? $t("common.copied") : $t("common.copy") }}</span>
          </button>
        </div>

        <div class="binary-links">
          <a
            class="binary-link"
            :href="config.app_url + '/dl/agents/windows/swm-agent.exe'"
            target="_blank"
            rel="noopener noreferrer"
          >
            <Download16 />
            <span>{{ $t("status.dl_win_agent") }}</span>
          </a>
          <a
            class="binary-link"
            :href="config.app_url + '/dl/agents/windows/swm-center.exe'"
            target="_blank"
            rel="noopener noreferrer"
          >
            <Download16 />
            <span>{{ $t("status.dl_win_center") }}</span>
          </a>
          <a
            class="binary-link"
            :href="config.app_url + '/dl/agents/linux-amd64/swm-agent'"
            target="_blank"
            rel="noopener noreferrer"
          >
            <Download16 />
            <span>{{ $t("status.dl_linux_agent") }}</span>
          </a>
        </div>
      </div>
    </cv-tile>

    <!-- Systemd Services Section -->
    <h4 class="section-heading">{{ $t("status.services") }}</h4>
    <div class="service-grid" v-if="status && status.services && status.services.length">
      <NsSystemdServiceCard
        v-for="service in status.services"
        :key="service.name"
        light
        :serviceName="service.name"
        :active="service.active"
        :failed="service.failed"
        :enabled="service.enabled"
        :icon="Cube32"
      />
    </div>
    <div v-else-if="!loadingStatus" class="mg-bottom-lg">
      <cv-tile light>{{ $t("status.no_services") }}</cv-tile>
    </div>

    <!-- Side-by-Side Container Images & Persistent Volumes -->
    <div class="two-col-grid">
      <cv-tile light>
        <h4 class="tile-heading">{{ $t("status.app_images") }}</h4>
        <div v-if="status && status.images && status.images.length" class="kv-list">
          <div v-for="img in status.images" :key="img.name" class="kv-row">
            <span class="kv-label kv-mono">{{ img.name }}</span>
            <span class="kv-value kv-mono">{{ formatImageSize(img.size) }}</span>
          </div>
        </div>
        <p v-else class="empty-note">{{ $t("status.no_images") }}</p>
      </cv-tile>

      <cv-tile light>
        <h4 class="tile-heading">{{ $t("status.app_volumes") }}</h4>
        <div v-if="status && status.volumes && status.volumes.length" class="kv-list">
          <div v-for="vol in status.volumes" :key="vol.name" class="kv-row">
            <span class="kv-label kv-mono">{{ vol.name }}</span>
            <span class="kv-value kv-mono">{{ formatVolumeTimestamp(vol) }}</span>
          </div>
        </div>
        <p v-else class="empty-note">{{ $t("status.no_volumes") }}</p>
      </cv-tile>
    </div>
  </div>
</template>

<script>
import { mapState, mapActions } from "vuex";
import to from "await-to-js";
import {
  QueryParamService,
  PageTitleService,
  TaskService,
  UtilService,
  IconService,
} from "@nethserver/ns8-ui-lib";
import Launch20 from "@carbon/icons-vue/es/launch/20";
import Settings20 from "@carbon/icons-vue/es/settings/20";
import Laptop20 from "@carbon/icons-vue/es/laptop/20";
import ArrowRight20 from "@carbon/icons-vue/es/arrow--right/20";
import Copy16 from "@carbon/icons-vue/es/copy/16";
import Checkmark16 from "@carbon/icons-vue/es/checkmark/16";
import Download16 from "@carbon/icons-vue/es/download/16";
import Application32 from "@carbon/icons-vue/es/application/32";
import EdgeNode32 from "@carbon/icons-vue/es/edge-node/32";
import Cube32 from "@carbon/icons-vue/es/cube/32";

export default {
  name: "Status",
  components: {
    Launch20,
    Settings20,
    Laptop20,
    ArrowRight20,
    Copy16,
    Checkmark16,
    Download16,
    Application32,
    EdgeNode32,
    Cube32,
  },
  mixins: [
    TaskService,
    QueryParamService,
    UtilService,
    IconService,
    PageTitleService,
  ],
  pageTitle() {
    return this.$t("status.title") + " - " + this.appName;
  },
  data() {
    return {
      Launch20,
      Settings20,
      Laptop20,
      ArrowRight20,
      Copy16,
      Checkmark16,
      Download16,
      Application32,
      EdgeNode32,
      Cube32,
      q: {
        page: "status",
      },
      urlCheckInterval: null,
      status: null,
      config: null,
      backupRepositories: [],
      backups: [],
      copiedUrl: false,
      loadingStatus: false,
      loadingConfig: false,
      loadingBackups: false,
    };
  },
  computed: {
    ...mapState(["instanceName", "instanceLabel", "core", "appName"]),
  },
  beforeRouteEnter(to, from, next) {
    next((vm) => {
      if (window.parent && window.parent.core && window.parent.core.$route) {
        vm.watchQueryData(vm);
        vm.urlCheckInterval = vm.initUrlBindingForApp(vm, vm.q.page);
      }
    });
  },
  beforeRouteLeave(to, from, next) {
    clearInterval(this.urlCheckInterval);
    next();
  },
  created() {
    if (this.core && this.instanceName) {
      this.getStatus();
      this.getConfiguration();
      this.listBackupRepositories();
    }
  },
  methods: {
    ...mapActions(["setConfiguredInStore", "setAppUrlInStore"]),
    openAdminConsole() {
      if (this.config && this.config.app_url) {
        window.open(this.config.app_url, "_blank", "noopener,noreferrer");
      }
    },
    openEnrollmentWizard() {
      if (this.config && this.config.app_url) {
        window.open(
          this.config.app_url.replace(/\/+$/, "") + "/enrollment",
          "_blank",
          "noopener,noreferrer"
        );
      }
    },
    async copyServerUrl() {
      if (!this.config || !this.config.app_url) {
        return;
      }
      try {
        if (navigator && navigator.clipboard) {
          await navigator.clipboard.writeText(this.config.app_url);
          this.copiedUrl = true;
          setTimeout(() => {
            this.copiedUrl = false;
          }, 2000);
        }
      } catch (e) {
        void e;
      }
    },
    formatImageSize(size) {
      if (size === null || size === undefined || size === "") {
        return "-";
      }
      if (typeof size === "string") {
        const trimmed = size.trim();
        if (!trimmed) {
          return "-";
        }
        const numeric = Number(trimmed);
        if (!Number.isFinite(numeric)) {
          return trimmed;
        }
        return this.formatBytesNumber(numeric);
      }
      if (typeof size === "number" && Number.isFinite(size)) {
        return this.formatBytesNumber(size);
      }
      return String(size);
    },
    formatBytesNumber(bytes) {
      if (bytes <= 0) {
        return "0 B";
      }
      const units = ["B", "KB", "MB", "GB", "TB"];
      let idx = 0;
      let val = bytes;
      while (val >= 1024 && idx < units.length - 1) {
        val /= 1024;
        idx += 1;
      }
      return `${val >= 10 || idx === 0 ? val.toFixed(0) : val.toFixed(1)} ${units[idx]}`;
    },
    formatVolumeTimestamp(vol) {
      if (!vol) {
        return "-";
      }
      const raw = vol.created || vol.mount || "";
      if (!raw) {
        return "-";
      }
      // Match Go timestamp e.g. "2026-11-23 11:58:28.210367265 +0100 CET"
      const goMatch = /^(\d{4}-\d{2}-\d{2})\s+(\d{2}:\d{2})/.exec(String(raw));
      if (goMatch) {
        return `${goMatch[1]} ${goMatch[2]}`;
      }
      const parsed = new Date(raw);
      if (!Number.isNaN(parsed.getTime())) {
        return parsed.toLocaleString();
      }
      return String(raw);
    },
    goToSettings() {
      if (this.core && this.core.$router) {
        this.goToAppPage(this.instanceName, "settings");
      } else {
        this.$router.push("/settings").catch(() => {});
      }
    },
    async getStatus() {
      this.loadingStatus = true;
      const taskAction = "get-status";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.getStatusAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.getStatusCompleted
      );

      const [err, res] = await to(
        this.createModuleTaskForApp(this.instanceName, {
          action: taskAction,
          extra: {
            title: this.$t("action." + taskAction),
            isNotificationHidden: true,
            eventId,
          },
        })
      );
      if (err) {
        this.loadingStatus = false;
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
      void res;
    },
    getStatusAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.loadingStatus = false;
    },
    getStatusCompleted(taskContext, taskResult) {
      this.status = taskResult.output;
      this.loadingStatus = false;
    },
    async getConfiguration() {
      this.loadingConfig = true;
      const taskAction = "get-configuration";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.getConfigurationAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.getConfigurationCompleted
      );

      const [err, res] = await to(
        this.createModuleTaskForApp(this.instanceName, {
          action: taskAction,
          extra: {
            title: this.$t("action." + taskAction),
            isNotificationHidden: true,
            eventId,
          },
        })
      );
      if (err) {
        this.loadingConfig = false;
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
      void res;
    },
    getConfigurationAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.loadingConfig = false;
    },
    getConfigurationCompleted(taskContext, taskResult) {
      const cfg = taskResult.output || {};
      this.config = cfg;
      this.setConfiguredInStore(Boolean(cfg.configured));
      this.setAppUrlInStore(cfg.app_url || "");
      this.loadingConfig = false;
    },
    async listBackupRepositories() {
      this.loadingBackups = true;
      const taskAction = "list-backup-repositories";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.listBackupRepositoriesAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.listBackupRepositoriesCompleted
      );

      const [err, res] = await to(
        this.createClusterTaskForApp({
          action: taskAction,
          extra: {
            title: this.$t("action." + taskAction),
            isNotificationHidden: true,
            eventId,
          },
        })
      );
      if (err) {
        this.loadingBackups = false;
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
      void res;
    },
    listBackupRepositoriesAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.loadingBackups = false;
    },
    async listBackupRepositoriesCompleted(taskContext, taskResult) {
      const out = (taskResult && taskResult.output) || {};
      this.backupRepositories = Array.isArray(out)
        ? out
        : Array.isArray(out.repositories)
        ? out.repositories
        : [];
      await this.listBackups();
    },
    async listBackups() {
      const taskAction = "list-backups";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.listBackupsAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.listBackupsCompleted
      );

      const [err, res] = await to(
        this.createClusterTaskForApp({
          action: taskAction,
          extra: {
            title: this.$t("action." + taskAction),
            isNotificationHidden: true,
            eventId,
          },
        })
      );
      if (err) {
        this.loadingBackups = false;
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
      void res;
    },
    listBackupsAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.loadingBackups = false;
    },
    listBackupsCompleted(taskContext, taskResult) {
      this.backups = (taskResult.output && taskResult.output.backups) || [];
      this.loadingBackups = false;
    },
  },
};
</script>

<style scoped lang="scss">
.page-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 1rem;
  margin-bottom: 1.5rem;
}

.toolbar-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.75rem;
}

.page-title {
  margin: 0;
}

.page-subtitle {
  margin: 0.2rem 0 0;
  font-size: 0.875rem;
  color: #525252;
}

.status-brand {
  display: flex;
  align-items: center;
  gap: 0.875rem;
}

.status-logo {
  width: 2.5rem;
  height: 2.5rem;
  flex-shrink: 0;
}

.card-cta {
  margin-top: 0.75rem;
  display: flex;
  justify-content: center;
}

.onboarding-tile {
  border-left: 4px solid #0f62fe;
  padding: 1.25rem 1.5rem;
}

.onboarding-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  flex-wrap: wrap;
  gap: 1rem;
  margin-bottom: 1.25rem;
}

.onboarding-title {
  margin: 0;
  font-size: 1.125rem;
  font-weight: 600;
  color: #161616;
}

.onboarding-subtitle {
  margin: 0.25rem 0 0;
  font-size: 0.875rem;
  color: #525252;
  max-width: 54rem;
}

.onboarding-steps {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1rem;
  margin-bottom: 1.25rem;
}

@media (max-width: 1100px) {
  .onboarding-steps {
    grid-template-columns: minmax(0, 1fr);
  }
}

.step-card {
  display: flex;
  align-items: flex-start;
  gap: 0.875rem;
  background-color: #f4f4f4;
  padding: 1rem;
  border: 1px solid #e0e0e0;
}

.step-badge {
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 9999px;
  background-color: #0f62fe;
  color: #ffffff;
  font-size: 0.875rem;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.step-heading {
  margin: 0 0 0.35rem;
  font-size: 0.9375rem;
  font-weight: 600;
  color: #161616;
}

.step-desc {
  margin: 0;
  font-size: 0.8125rem;
  line-height: 1.45;
  color: #525252;
}

.onboarding-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #e0e0e0;
}

.endpoint-copy-box {
  display: inline-flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.endpoint-label {
  font-size: 0.8125rem;
  font-weight: 600;
  color: #525252;
}

.endpoint-code {
  background-color: #e8e8e8;
  padding: 0.25rem 0.625rem;
  font-family: "IBM Plex Mono", monospace;
  font-size: 0.8125rem;
  color: #161616;
}

.copy-inline-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  border: 1px solid #8d8d8d;
  background-color: #ffffff;
  color: #161616;
  padding: 0.25rem 0.625rem;
  font-size: 0.75rem;
  font-weight: 500;
  cursor: pointer;
}

.copy-inline-btn:hover {
  background-color: #e8e8e8;
}

.binary-links {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.75rem;
}

.binary-link {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.8125rem;
  font-weight: 500;
  color: #0f62fe;
  text-decoration: none;
}

.binary-link:hover {
  text-decoration: underline;
}

.section-heading {
  margin: 0 0 0.75rem;
  font-size: 1rem;
  font-weight: 600;
}

.tile-heading {
  margin: 0 0 0.75rem;
  font-size: 0.9375rem;
  font-weight: 600;
}

.empty-note {
  color: #525252;
  font-size: 0.875rem;
}
</style>
