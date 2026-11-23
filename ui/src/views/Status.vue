<template>
  <cv-grid fullWidth>
    <cv-row>
      <cv-column class="page-title">
        <h2>{{ $t("status.title") }}</h2>
      </cv-column>
    </cv-row>

    <cv-row v-if="error.getStatus">
      <cv-column>
        <NsInlineNotification
          kind="error"
          :title="$t('action.get-status')"
          :description="error.getStatus"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>

    <cv-row v-if="error.getConfiguration">
      <cv-column>
        <NsInlineNotification
          kind="error"
          :title="$t('action.get-configuration')"
          :description="error.getConfiguration"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>

    <cv-row v-if="!loadingConfig && (!config || !config.configured)">
      <cv-column>
        <NsInlineNotification
          kind="warning"
          :title="$t('status.not_configured_title')"
          :description="$t('status.not_configured_description')"
          :actionLabel="$t('status.go_to_settings')"
          @action="goToSettings"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>

    <cv-row v-if="!loadingConfig && config && config.configured && config.app_url">
      <cv-column>
        <NsInlineNotification
          kind="info"
          :title="$t('status.onboarding_title')"
          :description="$t('status.onboarding_subtitle', { url: config.app_url })"
          :actionLabel="$t('status.open_rollout_wizard')"
          @action="openEnrollmentWizard"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>

    <!-- Summary Cards -->
    <cv-row>
      <cv-column :md="4" :max="4">
        <NsInfoCard
          light
          :wrapTitle="true"
          :title="
            config && config.domain_name
              ? config.domain_name
              : $t('status.not_configured_short')
          "
          :description="$t('status.configuration_summary')"
          :icon="Application32"
          :loading="loadingConfig"
          class="min-height-card"
        >
          <template slot="content">
            <div class="card-rows">
              <div v-if="config && config.app_url" class="card-row">
                <cv-link
                  :href="config.app_url"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {{ $t("status.open_webapp") }}
                  <Launch16 class="inline-launch" />
                </cv-link>
              </div>
              <div v-if="config && config.configured" class="card-tags">
                <NsTag
                  :label="
                    config.lets_encrypt
                      ? 'Let\'s Encrypt'
                      : $t('status.tls_default')
                  "
                  :kind="config.lets_encrypt ? 'green' : 'gray'"
                  size="sm"
                />
                <NsTag
                  :label="config.ldap_domain || $t('status.local_only')"
                  :kind="config.ldap_domain ? 'cyan' : 'gray'"
                  size="sm"
                />
              </div>
            </div>
          </template>
        </NsInfoCard>
      </cv-column>

      <cv-column :md="4" :max="4">
        <NsInfoCard
          light
          :wrapTitle="true"
          :title="
            status && status.node
              ? $t('status.node', { node: status.node })
              : '-'
          "
          :description="$t('status.installation_node')"
          :icon="Chip32"
          :loading="loadingStatus"
          class="min-height-card"
        >
          <template slot="content">
            <div class="card-rows">
              <div v-if="instanceName" class="card-meta">
                {{ $t("status.app_instance") }}: {{ instanceName }}
              </div>
            </div>
          </template>
        </NsInfoCard>
      </cv-column>

      <cv-column :md="4" :max="4">
        <NsBackupCard
          :title="core ? core.$t('backup.title') : 'Backup'"
          :noBackupMessage="
            core
              ? core.$t('backup.no_backup_configured')
              : 'No backup configured'
          "
          :goToBackupLabel="
            core ? core.$t('backup.go_to_backup') : 'Go to Backup'
          "
          :repositoryLabel="
            core ? core.$t('backup.repository') : 'Repository'
          "
          :statusLabel="core ? core.$t('common.status') : 'Status'"
          :statusSuccessLabel="core ? core.$t('common.success') : 'Success'"
          :statusNotRunLabel="
            core
              ? core.$t('backup.backup_has_not_run_yet')
              : 'Backup has not run yet'
          "
          :statusErrorLabel="core ? core.$t('error.error') : 'Error'"
          :completedLabel="
            core ? core.$t('backup.completed') : 'Completed'
          "
          :durationLabel="core ? core.$t('backup.duration') : 'Duration'"
          :totalSizeLabel="
            core ? core.$t('backup.total_size') : 'Total size'
          "
          :totalFileCountLabel="
            core ? core.$t('backup.total_file_count') : 'Total file count'
          "
          :backupDisabledLabel="
            core ? core.$t('common.disabled') : $t('common.disabled')
          "
          :showMoreLabel="
            core ? core.$t('common.show_more') : 'Show more'
          "
          :moduleId="instanceName"
          :moduleUiName="instanceLabel"
          :repositories="backupRepositories"
          :backups="backups"
          :loading="loadingBackups"
          :coreContext="core"
          light
        />
      </cv-column>

      <cv-column :md="4" :max="4">
        <NsSystemLogsCard
          :title="
            core
              ? core.$t('system_logs.card_title') || core.$t('system_logs.title')
              : 'System logs'
          "
          :description="
            core
              ? core.$t('system_logs.card_description', {
                  name: instanceLabel || instanceName,
                })
              : instanceName
          "
          :buttonLabel="
            core ? core.$t('system_logs.card_button_label') : 'Open logs'
          "
          :router="core ? core.$router : null"
          context="module"
          :moduleId="instanceName"
          light
        />
      </cv-column>
    </cv-row>

    <!-- Services -->
    <cv-row>
      <cv-column class="page-subtitle">
        <h4>{{ $t("status.services") }}</h4>
      </cv-column>
    </cv-row>
    <cv-row v-if="!loadingStatus">
      <cv-column v-if="!status || !status.services || !status.services.length">
        <cv-tile light>
          <NsEmptyState :title="$t('status.no_services')" />
        </cv-tile>
      </cv-column>
      <cv-column
        v-else
        v-for="(service, index) in status.services"
        :key="index"
        :md="4"
        :max="4"
      >
        <NsSystemdServiceCard
          light
          class="min-height-card"
          :serviceName="service.name"
          :active="service.active"
          :failed="service.failed"
          :enabled="service.enabled"
          :icon="Cube32"
        />
      </cv-column>
    </cv-row>
    <cv-row v-else>
      <cv-column :md="4" :max="4">
        <cv-tile light>
          <cv-skeleton-text :paragraph="true" :line-count="4" />
        </cv-tile>
      </cv-column>
    </cv-row>

    <!-- Images -->
    <cv-row>
      <cv-column class="page-subtitle">
        <h4>{{ $t("status.app_images") }}</h4>
      </cv-column>
    </cv-row>
    <cv-row>
      <cv-column>
        <cv-tile light>
          <div v-if="!loadingStatus">
            <NsEmptyState
              v-if="!status || !status.images || !status.images.length"
              :title="$t('status.no_images')"
            />
            <cv-structured-list v-else>
              <template slot="headings">
                <cv-structured-list-heading>{{
                  $t("status.name")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.size")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.created")
                }}</cv-structured-list-heading>
              </template>
              <template slot="items">
                <cv-structured-list-item
                  v-for="(image, index) in status.images"
                  :key="index"
                >
                  <cv-structured-list-data class="break-word">{{
                    image.name
                  }}</cv-structured-list-data>
                  <cv-structured-list-data>{{
                    formatImageSize(image.size)
                  }}</cv-structured-list-data>
                  <cv-structured-list-data class="break-word">{{
                    formatTimestamp(image.created)
                  }}</cv-structured-list-data>
                </cv-structured-list-item>
              </template>
            </cv-structured-list>
          </div>
          <cv-skeleton-text v-else :paragraph="true" :line-count="5" />
        </cv-tile>
      </cv-column>
    </cv-row>

    <!-- Volumes -->
    <cv-row>
      <cv-column class="page-subtitle">
        <h4>{{ $t("status.app_volumes") }}</h4>
      </cv-column>
    </cv-row>
    <cv-row>
      <cv-column>
        <cv-tile light>
          <div v-if="!loadingStatus">
            <NsEmptyState
              v-if="!status || !status.volumes || !status.volumes.length"
              :title="$t('status.no_volumes')"
            />
            <cv-structured-list v-else>
              <template slot="headings">
                <cv-structured-list-heading>{{
                  $t("status.name")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.mount")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.created")
                }}</cv-structured-list-heading>
              </template>
              <template slot="items">
                <cv-structured-list-item
                  v-for="(volume, index) in status.volumes"
                  :key="index"
                >
                  <cv-structured-list-data>{{
                    volume.name
                  }}</cv-structured-list-data>
                  <cv-structured-list-data class="break-word">{{
                    volume.mount || "-"
                  }}</cv-structured-list-data>
                  <cv-structured-list-data>{{
                    formatTimestamp(volume.created)
                  }}</cv-structured-list-data>
                </cv-structured-list-item>
              </template>
            </cv-structured-list>
          </div>
          <cv-skeleton-text v-else :paragraph="true" :line-count="5" />
        </cv-tile>
      </cv-column>
    </cv-row>
  </cv-grid>
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
import Launch16 from "@carbon/icons-vue/es/launch/16";
import Application32 from "@carbon/icons-vue/es/application/32";
import Chip32 from "@carbon/icons-vue/es/chip/32";
import Cube32 from "@carbon/icons-vue/es/cube/32";

export default {
  name: "Status",
  components: {
    Launch16,
    Application32,
    Chip32,
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
      Launch16,
      Application32,
      Chip32,
      Cube32,
      q: {
        page: "status",
      },
      urlCheckInterval: null,
      status: null,
      config: null,
      backupRepositories: [],
      backups: [],
      loadingStatus: false,
      loadingConfig: false,
      loadingBackups: false,
      error: {
        getStatus: "",
        getConfiguration: "",
      },
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
    openEnrollmentWizard() {
      if (this.config && this.config.app_url) {
        window.open(
          this.config.app_url.replace(/\/+$/, "") + "/enrollment",
          "_blank",
          "noopener,noreferrer"
        );
      }
    },
    goToSettings() {
      if (this.core && this.core.$router) {
        this.goToAppPage(this.instanceName, "settings");
      } else {
        this.$router.push("/settings").catch(() => {});
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
    formatTimestamp(raw) {
      if (!raw) {
        return "-";
      }
      const goMatch = /^(\d{4}-\d{2}-\d{2})\s+(\d{2}:\d{2})/.exec(String(raw));
      if (goMatch) {
        return `${goMatch[1]} ${goMatch[2]}`;
      }
      return String(raw);
    },
    async getStatus() {
      this.loadingStatus = true;
      this.error.getStatus = "";
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
        this.error.getStatus = this.$t("error.cannot_retrieve_status");
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
      this.error.getStatus = this.$t("error.cannot_retrieve_status");
      this.loadingStatus = false;
    },
    getStatusCompleted(taskContext, taskResult) {
      this.status = taskResult.output;
      this.loadingStatus = false;
    },
    async getConfiguration() {
      this.loadingConfig = true;
      this.error.getConfiguration = "";
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
        this.error.getConfiguration = this.$t("error.cannot_retrieve_configuration");
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
      this.error.getConfiguration = this.$t("error.cannot_retrieve_configuration");
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
      const backups = (taskResult.output && taskResult.output.backups) || [];
      for (const backup of backups) {
        const repo = this.backupRepositories.find(
          (r) => r.id === backup.repository
        );
        if (repo) {
          backup.repoName = repo.name;
        }
      }
      this.backups = backups;
      this.loadingBackups = false;
    },
  },
};
</script>

<style scoped lang="scss">
@import "../styles/base";

.break-word {
  word-wrap: break-word;
  max-width: 30vw;
}

.card-rows {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  width: 100%;
  gap: $spacing-03;
  margin-top: $spacing-02;
}

.card-row {
  margin-top: $spacing-02;
}

.card-tags {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: $spacing-02;
}

.card-meta {
  font-size: 0.875rem;
  color: $text-02;
}

.inline-launch {
  vertical-align: middle;
  margin-left: $spacing-02;
}
</style>
