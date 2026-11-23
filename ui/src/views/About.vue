<template>
  <cv-grid fullWidth>
    <cv-row>
      <cv-column class="page-title">
        <h2>{{ $t("about.title") }}</h2>
      </cv-column>
    </cv-row>

    <cv-row v-if="error.version">
      <cv-column>
        <NsInlineNotification
          kind="error"
          :title="$t('error.cannot_retrieve_installed_modules')"
          :description="error.version"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>

    <cv-row>
      <cv-column>
        <cv-tile light>
          <cv-skeleton-text
            v-if="loading.moduleInfo"
            :paragraph="true"
            :line-count="6"
          />

          <div v-else-if="app">
            <div class="logo-and-name">
              <div class="app-logo">
                <img src="assets/module_default_logo.svg" :alt="app.name" />
              </div>
              <h3>{{ app.name }}</h3>
            </div>

            <div class="description">
              {{ getApplicationDescription(app) }}
            </div>

            <section>
              <span class="section-title">{{ $t("about.version") }}: </span>
              <cv-skeleton-text
                v-if="loading.version"
                class="version-skeleton"
              />
              <span v-else>{{ version || "1.0.6" }}</span>
            </section>

            <section v-if="app.categories && app.categories.length">
              <span class="section-title">{{ $t("about.categories") }}: </span>
              <span>{{ app.categories.join(", ") }}</span>
            </section>

            <section v-if="app.docs && app.docs.documentation_url">
              <span class="section-title"
                >{{ $t("about.documentation") }}:
              </span>
              <cv-link :href="app.docs.documentation_url" target="_blank">
                {{ app.docs.documentation_url }}
              </cv-link>
            </section>

            <section v-if="app.docs && app.docs.bug_url">
              <span class="section-title"
                >{{ $t("about.bug_tracker") }}:
              </span>
              <cv-link :href="app.docs.bug_url" target="_blank">
                {{ app.docs.bug_url }}
              </cv-link>
            </section>

            <section v-if="app.docs && app.docs.code_url">
              <span class="section-title"
                >{{ $t("about.source_code") }}:
              </span>
              <cv-link :href="app.docs.code_url" target="_blank">
                {{ app.docs.code_url }}
              </cv-link>
            </section>

            <section v-if="app.authors && app.authors.length">
              <span class="section-title">{{ $t("about.authors") }}: </span>
              <span v-if="app.authors.length === 1">
                {{ app.authors[0].name }}
                <cv-link
                  v-if="app.authors[0].email"
                  :href="'mailto:' + app.authors[0].email"
                  target="_blank"
                  class="email"
                >
                  {{ app.authors[0].email }}
                </cv-link>
              </span>
              <ul v-else class="authors">
                <li
                  v-for="(author, idx) in app.authors"
                  :key="idx"
                  class="author"
                >
                  {{ author.name }}
                  <cv-link
                    v-if="author.email"
                    :href="'mailto:' + author.email"
                    target="_blank"
                    class="email"
                  >
                    {{ author.email }}
                  </cv-link>
                </li>
              </ul>
            </section>

            <section>
              <span class="section-title">{{ $t("about.license") }}: </span>
              <span>GPL-3.0-or-later</span>
            </section>
          </div>
        </cv-tile>
      </cv-column>
    </cv-row>
  </cv-grid>
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

export default {
  name: "About",
  components: {},
  mixins: [TaskService, UtilService, PageTitleService, QueryParamService],
  pageTitle() {
    return this.$t("about.title") + " - " + this.appName;
  },
  data() {
    return {
      q: {
        page: "about",
      },
      urlCheckInterval: null,
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
        this.error.version = this.$t(
          "error.cannot_retrieve_installed_modules"
        );
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
@import "../styles/base";

.logo-and-name {
  display: flex;
  align-items: center;
  margin-top: $spacing-05;
  margin-bottom: $spacing-07;
}

.app-logo {
  max-width: 4rem;
  max-height: 4rem;
  margin-right: $spacing-05;
  flex-shrink: 0;
}

.app-logo img {
  width: 100%;
  height: 100%;
}

.description {
  margin-bottom: $spacing-07;
}

section {
  margin-bottom: $spacing-05;
}

.section-title {
  font-weight: bold;
}

.author {
  margin-left: $spacing-05;
  margin-bottom: $spacing-02;
}

.authors {
  margin-top: $spacing-02;
}

.email {
  margin-left: $spacing-02;
}

.version-skeleton {
  display: inline-flex;
  width: 3rem !important;
  position: relative;
  top: 1px;
}
</style>
