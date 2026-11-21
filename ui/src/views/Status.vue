<template>
  <div class="bx--grid bx--grid--full-width ns-page-content">
    <div class="bx--row">
      <div class="bx--col-lg-16 page-toolbar">
        <div class="status-brand">
          <img
            class="status-logo"
            src="assets/module_default_logo.svg"
            alt="SoftwareManager"
          />
          <h2 class="page-title">{{ $t("status.title") }}</h2>
        </div>
        <NsButton
          v-if="config && config.app_url"
          kind="secondary"
          :icon="Launch20"
          @click="openAdminConsole"
        >
          {{ $t("status.open_webapp") }}
        </NsButton>
      </div>
    </div>

    <!-- Unconfigured banner -->
    <div v-if="!loadingConfig && (!config || !config.configured)" class="bx--row mg-bottom-lg">
      <div class="bx--col-lg-16">
        <NsInlineNotification
          kind="warning"
          :title="$t('status.not_configured_title')"
          :description="$t('status.not_configured_description')"
          :actionLabel="$t('status.go_to_settings')"
          @action="goToSettings"
          :showCloseButton="false"
        />
      </div>
    </div>

    <!-- Instance & Routing Summary Cards -->
    <div class="card-grid">
      <!-- App Instance Card -->
      <NsInfoCard
        light
        :title="instanceLabel || instanceName || 'softwaremanager'"
        :description="$t('status.app_instance')"
        :icon="Application32"
        :loading="loadingStatus"
        class="ns-card"
      >
        <template slot="content">
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
            <div v-if="config && config.app_url" class="mg-top-md">
              <NsButton
                kind="ghost"
                :icon="Launch20"
                @click="openAdminConsole"
              >
                {{ $t("status.open_webapp") }}
              </NsButton>
            </div>
            <div v-else class="mg-top-md">
              <NsButton
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

      <!-- Endpoint & Portal Routing Card -->
      <NsInfoCard
        light
        :title="$t('status.configuration_summary')"
        :description="config && config.domain_name ? config.domain_name : '-'"
        :icon="EdgeNode32"
        :loading="loadingConfig"
        class="ns-card"
      >
        <template slot="content">
          <div class="kv-list" v-if="config">
            <div class="kv-row">
              <span class="kv-label">{{ $t("status.domain_name") }}</span>
              <span class="kv-value">{{ config.domain_name || "-" }}</span>
            </div>
            <div class="kv-row">
              <span class="kv-label">{{ $t("status.lets_encrypt") }}</span>
              <span class="kv-value">
                {{ config.lets_encrypt ? $t("common.enabled") : $t("common.disabled") }}
              </span>
            </div>
            <div class="kv-row">
              <span class="kv-label">{{ $t("status.http2https") }}</span>
              <span class="kv-value">
                {{ config.http2https ? $t("common.enabled") : $t("common.disabled") }}
              </span>
            </div>
            <div class="kv-row">
              <span class="kv-label">{{ $t("status.ldap_domain") }}</span>
              <span class="kv-value">
                {{ config.ldap_domain || $t("status.local_only") }}
              </span>
            </div>
            <div class="kv-row">
              <span class="kv-label">{{ $t("status.admin_username") }}</span>
              <span class="kv-value">{{ config.admin_username || "admin" }}</span>
            </div>
          </div>
        </template>
      </NsInfoCard>

      <!-- Backup Card -->
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

      <!-- System Logs Card -->
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

    <!-- Systemd Services Section -->
    <div class="bx--row mg-bottom-md">
      <div class="bx--col-lg-16">
        <h4 class="section-heading">{{ $t("status.services") }}</h4>
      </div>
    </div>
    <div class="card-grid" v-if="status && status.services && status.services.length">
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
    <div v-else-if="!loadingStatus" class="bx--row mg-bottom-lg">
      <div class="bx--col-lg-16">
        <cv-tile light>{{ $t("status.no_services") }}</cv-tile>
      </div>
    </div>

    <!-- Container Images & Volumes -->
    <div class="bx--row mg-bottom-lg">
      <div class="bx--col-lg-8 mg-bottom-md">
        <cv-tile light>
          <h4 class="tile-heading">{{ $t("status.app_images") }}</h4>
          <div v-if="status && status.images && status.images.length" class="kv-list">
            <div v-for="img in status.images" :key="img.name" class="kv-row">
              <span class="kv-label">{{ img.name }}</span>
              <span class="kv-value">{{ img.size | byteFormat }}</span>
            </div>
          </div>
          <p v-else class="empty-note">{{ $t("status.no_images") }}</p>
        </cv-tile>
      </div>

      <div class="bx--col-lg-8 mg-bottom-md">
        <cv-tile light>
          <h4 class="tile-heading">{{ $t("status.app_volumes") }}</h4>
          <div v-if="status && status.volumes && status.volumes.length" class="kv-list">
            <div v-for="vol in status.volumes" :key="vol.name" class="kv-row">
              <span class="kv-label">{{ vol.name }}</span>
              <span class="kv-value">{{ vol.created || vol.mount || "-" }}</span>
            </div>
          </div>
          <p v-else class="empty-note">{{ $t("status.no_volumes") }}</p>
        </cv-tile>
      </div>
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
import Application32 from "@carbon/icons-vue/es/application/32";
import EdgeNode32 from "@carbon/icons-vue/es/edge-node/32";
import Cube32 from "@carbon/icons-vue/es/cube/32";

export default {
  name: "Status",
  components: {
    Launch20,
    Settings20,
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
      Application32,
      EdgeNode32,
      Cube32,
      status: null,
      config: null,
      backupRepositories: [],
      backups: [],
      loadingStatus: false,
      loadingConfig: false,
      loadingBackups: false,
    };
  },
  computed: {
    ...mapState(["instanceName", "instanceLabel", "core", "appName"]),
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
    goToSettings() {
      if (this.core && this.core.$router) {
        this.core.$router.push(`/apps/${this.instanceName}/settings`);
      } else {
        this.$router.push("/settings");
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
      this.backupRepositories = taskResult.output || [];
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

.page-title {
  margin: 0;
}

.status-brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.status-logo {
  width: 2.25rem;
  height: 2.25rem;
  flex-shrink: 0;
}

.section-heading {
  margin-bottom: 0.5rem;
}

.tile-heading {
  margin-bottom: 0.75rem;
}

.empty-note {
  color: #525252;
  font-size: 0.875rem;
}
</style>
