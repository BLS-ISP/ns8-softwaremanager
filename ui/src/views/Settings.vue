<template>
  <div class="bx--grid bx--grid--full-width ns-page-content">
    <div class="bx--row">
      <div class="bx--col-lg-16 page-toolbar">
        <h2 class="page-title">{{ $t("settings.title") }}</h2>
        <NsButton
          v-if="appUrl"
          kind="secondary"
          :icon="Launch20"
          @click="openAdminConsole"
        >
          {{ $t("settings.open_console") }}
        </NsButton>
      </div>
    </div>

    <div v-if="saveSuccess" class="bx--row mg-bottom-md">
      <div class="bx--col-lg-10">
        <NsInlineNotification
          kind="success"
          :title="$t('settings.saved_title')"
          :description="$t('settings.saved_description')"
          :showCloseButton="true"
          @close="saveSuccess = false"
        />
      </div>
    </div>

    <div v-if="error.getConfiguration" class="bx--row mg-bottom-md">
      <div class="bx--col-lg-10">
        <NsInlineNotification
          kind="error"
          :title="$t('action.get-configuration')"
          :description="error.getConfiguration"
          :showCloseButton="false"
        />
      </div>
    </div>

    <div v-if="error.configureModule" class="bx--row mg-bottom-md">
      <div class="bx--col-lg-10">
        <NsInlineNotification
          kind="error"
          :title="$t('action.configure-module')"
          :description="error.configureModule"
          :showCloseButton="false"
        />
      </div>
    </div>

    <div class="bx--row">
      <div class="bx--col-lg-10">
        <cv-tile light>
          <cv-skeleton-text
            v-if="loading.getConfiguration"
            :paragraph="true"
            :line-count="8"
          />

          <cv-form v-else @submit.prevent="configureModule">
            <!-- Hostname / FQDN -->
            <NsTextInput
              v-model.trim="domain_name"
              :label="$t('settings.domain_name')"
              :placeholder="$t('settings.domain_name_placeholder')"
              :helper-text="$t('settings.domain_name_helper')"
              :invalid-message="error.domain_name"
              :disabled="loading.configureModule"
              ref="domain_name"
            />

            <!-- Let's Encrypt toggle -->
            <NsToggle
              v-model="lets_encrypt"
              :label="$t('settings.lets_encrypt')"
              value="lets_encrypt"
              :form-item="true"
              :disabled="loading.configureModule"
              class="mg-bottom-md"
            >
              <template slot="text-left">{{ $t("common.disabled") }}</template>
              <template slot="text-right">{{ $t("common.enabled") }}</template>
            </NsToggle>

            <!-- HTTP to HTTPS redirect toggle -->
            <NsToggle
              v-model="http2https"
              :label="$t('settings.http2https')"
              value="http2https"
              :form-item="true"
              :disabled="loading.configureModule"
              class="mg-bottom-md"
            >
              <template slot="text-left">{{ $t("common.disabled") }}</template>
              <template slot="text-right">{{ $t("common.enabled") }}</template>
            </NsToggle>

            <!-- NS8 User-Domain / LDAP selector -->
            <NsComboBox
              v-model="ldap_domain"
              :title="$t('settings.ldap_domain')"
              :label="$t('settings.ldap_domain_placeholder')"
              :helper-text="$t('settings.ldap_domain_helper')"
              :invalid-message="error.ldap_domain"
              :auto-filter="true"
              :auto-highlight="true"
              :options="userDomainOptions"
              :disabled="loading.configureModule"
              class="mg-bottom-md"
            />

            <!-- Local fallback administrator username -->
            <NsTextInput
              v-model.trim="admin_username"
              :label="$t('settings.admin_username')"
              :helper-text="$t('settings.admin_username_helper')"
              :invalid-message="error.admin_username"
              :disabled="loading.configureModule"
              ref="admin_username"
            />

            <!-- Local fallback administrator password (optional update) -->
            <NsTextInput
              v-model="admin_password"
              type="password"
              :label="$t('settings.admin_password')"
              :placeholder="$t('settings.admin_password_placeholder')"
              :helper-text="$t('settings.admin_password_helper')"
              :invalid-message="error.admin_password"
              :disabled="loading.configureModule"
              ref="admin_password"
            />

            <div class="form-actions">
              <NsButton
                kind="primary"
                :icon="Save20"
                :loading="loading.configureModule"
                :disabled="loading.getConfiguration || loading.configureModule"
              >
                {{ $t("settings.save") }}
              </NsButton>

              <NsButton
                v-if="appUrl"
                type="button"
                kind="ghost"
                :icon="Launch20"
                :disabled="loading.configureModule"
                @click="openAdminConsole"
              >
                {{ $t("settings.open_console") }}
              </NsButton>
            </div>
          </cv-form>
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
import Save20 from "@carbon/icons-vue/es/save/20";
import Launch20 from "@carbon/icons-vue/es/launch/20";

export default {
  name: "Settings",
  components: {
    Save20,
    Launch20,
  },
  mixins: [
    TaskService,
    IconService,
    UtilService,
    QueryParamService,
    PageTitleService,
  ],
  pageTitle() {
    return this.$t("settings.title") + " - " + this.appName;
  },
  data() {
    return {
      Save20,
      Launch20,
      domain_name: "",
      lets_encrypt: false,
      http2https: true,
      ldap_domain: "",
      userDomains: [],
      admin_username: "admin",
      admin_password: "",
      appUrl: "",
      saveSuccess: false,
      loading: {
        getConfiguration: false,
        configureModule: false,
      },
      error: {
        getConfiguration: "",
        configureModule: "",
        domain_name: "",
        ldap_domain: "",
        admin_username: "",
        admin_password: "",
      },
    };
  },
  computed: {
    ...mapState(["instanceName", "core", "appName"]),
    userDomainOptions() {
      const options = [
        {
          name: "local_only",
          label: this.$t("settings.ldap_local_only"),
          value: "",
        },
      ];
      for (const domain of this.userDomains) {
        if (domain && domain.name) {
          const schemaLabel = domain.schema ? ` (${domain.schema.toUpperCase()})` : "";
          options.push({
            name: domain.name,
            label: `${domain.name}${schemaLabel}`,
            value: domain.name,
          });
        }
      }
      return options;
    },
  },
  created() {
    if (this.core && this.instanceName) {
      this.getConfiguration();
    }
  },
  methods: {
    ...mapActions(["setConfiguredInStore", "setAppUrlInStore"]),
    openAdminConsole() {
      if (this.appUrl) {
        window.open(this.appUrl, "_blank", "noopener,noreferrer");
      }
    },
    clearErrors() {
      this.error.getConfiguration = "";
      this.error.configureModule = "";
      this.error.domain_name = "";
      this.error.ldap_domain = "";
      this.error.admin_username = "";
      this.error.admin_password = "";
    },
    validateConfigureModule() {
      this.clearErrors();
      let isValidationOk = true;

      if (!this.domain_name) {
        this.error.domain_name = this.$t("error.domain_name_required");
        isValidationOk = false;
      } else if (!/^[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/.test(this.domain_name)) {
        this.error.domain_name = this.$t("error.invalid_domain_name");
        isValidationOk = false;
      }

      if (!this.admin_username) {
        this.error.admin_username = this.$t("common.required");
        isValidationOk = false;
      }

      return isValidationOk;
    },
    async getConfiguration() {
      this.loading.getConfiguration = true;
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
        this.error.getConfiguration = this.$t("error.cannot_retrieve_configuration");
        this.loading.getConfiguration = false;
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
      this.loading.getConfiguration = false;
    },
    getConfigurationCompleted(taskContext, taskResult) {
      const config = taskResult.output || {};
      this.domain_name = config.domain_name || "";
      this.lets_encrypt = Boolean(config.lets_encrypt);
      this.http2https = config.http2https !== undefined ? Boolean(config.http2https) : true;
      this.ldap_domain = config.ldap_domain || "";
      this.userDomains = Array.isArray(config.user_domains) ? config.user_domains : [];
      this.admin_username = config.admin_username || "admin";
      this.admin_password = "";
      this.appUrl = config.app_url || "";
      this.setConfiguredInStore(Boolean(config.configured));
      this.setAppUrlInStore(this.appUrl);
      this.loading.getConfiguration = false;
    },
    async configureModule() {
      this.saveSuccess = false;
      if (!this.validateConfigureModule()) {
        return;
      }

      this.loading.configureModule = true;
      const taskAction = "configure-module";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.configureModuleAborted
      );
      this.core.$root.$once(
        `${taskAction}-validation-failed-${eventId}`,
        this.configureModuleValidationFailed
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.configureModuleCompleted
      );

      const payload = {
        domain_name: this.domain_name.toLowerCase(),
        lets_encrypt: this.lets_encrypt,
        http2https: this.http2https,
        ldap_domain: this.ldap_domain || "",
        admin_username: this.admin_username,
      };
      if (this.admin_password) {
        payload.admin_password = this.admin_password;
      }

      const [err, res] = await to(
        this.createModuleTaskForApp(this.instanceName, {
          action: taskAction,
          data: payload,
          extra: {
            title: this.$t("settings.configure_instance", {
              instance: this.instanceName,
            }),
            description: this.$t("common.processing"),
            eventId,
          },
        })
      );
      if (err) {
        this.loading.configureModule = false;
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
      void res;
    },
    configureModuleValidationFailed(validationErrors) {
      this.loading.configureModule = false;
      for (const validationError of validationErrors || []) {
        const param = validationError.parameter || validationError.field;
        if (param && Object.prototype.hasOwnProperty.call(this.error, param)) {
          this.error[param] = this.$t("error." + validationError.error);
        }
      }
    },
    configureModuleAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.error.configureModule = this.$t("error.generic_error");
      this.loading.configureModule = false;
    },
    configureModuleCompleted(taskContext, taskResult) {
      this.loading.configureModule = false;
      this.admin_password = "";
      this.saveSuccess = true;
      if (taskResult && taskResult.output && taskResult.output.app_url) {
        this.appUrl = taskResult.output.app_url;
        this.setAppUrlInStore(this.appUrl);
        this.setConfiguredInStore(true);
      }
      this.getConfiguration();
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

.form-actions {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-top: 1.5rem;
}
</style>
