<template>
  <div class="bx--grid bx--grid--full-width ns-page-content">
    <div class="bx--row">
      <div class="bx--col-lg-16 mg-bottom-lg">
        <h2>{{ $t("about.title") }}</h2>
      </div>
    </div>

    <div v-if="error.version" class="bx--row mg-bottom-md">
      <div class="bx--col-lg-10">
        <NsInlineNotification
          kind="error"
          :title="$t('error.cannot_retrieve_installed_modules')"
          :description="error.version"
          :showCloseButton="false"
        />
      </div>
    </div>

    <div class="bx--row">
      <div class="bx--col-lg-10">
        <cv-tile light>
          <cv-skeleton-text
            v-if="loading.moduleInfo"
            :paragraph="true"
            :line-count="6"
          />

          <div v-else-if="app">
            <div class="about-header">
              <img
                class="app-logo"
                src="assets/module_default_logo.svg"
                :alt="app.name"
              />
              <div>
                <h3>{{ app.name }}</h3>
                <p class="app-description">{{ getApplicationDescription(app) }}</p>
              </div>
            </div>

            <div class="kv-list">
              <div class="kv-row">
                <span class="kv-label">{{ $t("about.version") }}</span>
                <span class="kv-value">{{ version || "1.0.0" }}</span>
              </div>

              <div class="kv-row" v-if="app.categories && app.categories.length">
                <span class="kv-label">{{ $t("about.categories") }}</span>
                <span class="kv-value">{{ app.categories.join(", ") }}</span>
              </div>

              <div class="kv-row" v-if="app.docs && app.docs.documentation_url">
                <span class="kv-label">{{ $t("about.documentation") }}</span>
                <span class="kv-value">
                  <cv-link :href="app.docs.documentation_url" target="_blank">
                    {{ app.docs.documentation_url }}
                    <Launch20 class="inline-icon" />
                  </cv-link>
                </span>
              </div>

              <div class="kv-row" v-if="app.docs && app.docs.bug_url">
                <span class="kv-label">{{ $t("about.bug_tracker") }}</span>
                <span class="kv-value">
                  <cv-link :href="app.docs.bug_url" target="_blank">
                    {{ app.docs.bug_url }}
                    <Launch20 class="inline-icon" />
                  </cv-link>
                </span>
              </div>

              <div class="kv-row" v-if="app.docs && app.docs.code_url">
                <span class="kv-label">{{ $t("about.source_code") }}</span>
                <span class="kv-value">
                  <cv-link :href="app.docs.code_url" target="_blank">
                    {{ app.docs.code_url }}
                    <Launch20 class="inline-icon" />
                  </cv-link>
                </span>
              </div>

              <div class="kv-row" v-if="app.authors && app.authors.length">
                <span class="kv-label">{{ $t("about.authors") }}</span>
                <span class="kv-value">
                  <span v-for="(author, idx) in app.authors" :key="idx">
                    {{ author.name }} ({{ author.email }})<span
                      v-if="idx < app.authors.length - 1"
                      >,
                    </span>
                  </span>
                </span>
              </div>

              <div class="kv-row">
                <span class="kv-label">{{ $t("about.license") }}</span>
                <span class="kv-value">GPL-3.0-or-later</span>
              </div>
            </div>
          </div>
        </cv-tile>
      </div>
    </div>
  </div>
</template>

<script>
import { mapState } from "vuex";
import axios from "axios";
import to from "await-to-js";
import {
  TaskService,
  UtilService,
  PageTitleService,
  QueryParamService,
} from "@nethserver/ns8-ui-lib";
import Launch20 from "@carbon/icons-vue/es/launch/20";

export default {
  name: "About",
  components: {
    Launch20,
  },
  mixins: [TaskService, UtilService, PageTitleService, QueryParamService],
  pageTitle() {
    return this.$t("about.title") + " - " + this.appName;
  },
  data() {
    return {
      Launch20,
      app: null,
      version: "",
      loading: {
        moduleInfo: false,
        version: false,
      },
      error: {
        moduleInfo: "",
        version: "",
      },
    };
  },
  computed: {
    ...mapState(["instanceName", "core", "appName"]),
  },
  created() {
    this.getModuleInfo();
    if (this.core && this.instanceName) {
      this.listInstalledModules();
    }
  },
  methods: {
    getApplicationDescription(app) {
      if (!app || !app.description) {
        return "";
      }
      if (typeof app.description === "string") {
        return app.description;
      }
      const locale = this.$i18n.locale || "en";
      return app.description[locale] || app.description.en || "";
    },
    async getModuleInfo() {
      this.loading.moduleInfo = true;
      try {
        const response = await axios.get("metadata.json");
        this.app = response.data;
      } catch (err) {
        console.error(err);
        this.error.moduleInfo = this.$t("error.cannot_retrieve_module_info");
      } finally {
        this.loading.moduleInfo = false;
      }
    },
    async listInstalledModules() {
      this.loading.version = true;
      const taskAction = "list-installed-modules";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.listInstalledModulesAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.listInstalledModulesCompleted
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
        this.loading.version = false;
        this.error.version = this.$t("error.cannot_retrieve_installed_modules");
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
      void res;
    },
    listInstalledModulesAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.error.version = this.$t("error.cannot_retrieve_installed_modules");
      this.loading.version = false;
    },
    listInstalledModulesCompleted(taskContext, taskResult) {
      const modulesByImage = taskResult.output || {};
      for (const instances of Object.values(modulesByImage)) {
        for (const inst of instances || []) {
          if (inst.id === this.instanceName) {
            this.version = inst.version || "";
          }
        }
      }
      this.loading.version = false;
    },
  },
};
</script>

<style scoped lang="scss">
.about-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1.5rem;
}

.app-logo {
  width: 3.5rem;
  height: 3.5rem;
  flex-shrink: 0;
}

.app-description {
  color: #525252;
  margin-top: 0.25rem;
}

.inline-icon {
  vertical-align: middle;
  margin-left: 0.25rem;
}
</style>
