<template>
  <div id="ns8-app" class="bx--body">
    <cv-side-nav id="app-side-nav" :fixed="true" :expanded="true" class="app-side-nav">
      <AppSideMenuContent />
    </cv-side-nav>
    <AppMobileSideMenu />
    <cv-content id="main-content" class="ns-app-main-content">
      <router-view />
    </cv-content>
  </div>
</template>

<script>
import { mapState, mapActions } from "vuex";
import {
  QueryParamService,
  TaskService,
  UtilService,
} from "@nethserver/ns8-ui-lib";
import to from "await-to-js";
import AppSideMenuContent from "./components/AppSideMenuContent.vue";
import AppMobileSideMenu from "./components/AppMobileSideMenu.vue";

export default {
  name: "App",
  components: {
    AppSideMenuContent,
    AppMobileSideMenu,
  },
  mixins: [QueryParamService, TaskService, UtilService],
  computed: {
    ...mapState(["instanceName", "core"]),
    coreLocale() {
      return this.core && this.core.$i18n ? this.core.$i18n.locale : null;
    },
  },
  watch: {
    coreLocale(newLang) {
      if (!newLang) {
        return;
      }
      const normalized = String(newLang).substring(0, 2).toLowerCase();
      if (normalized === "en" || normalized === "de") {
        this.$i18n.locale = normalized;
      }
    },
  },
  created() {
    const core = window.parent && window.parent.core ? window.parent.core : null;
    this.setCoreInStore(core);

    let instanceName = "softwaremanager1";
    const hashMatch =
      window.parent &&
      window.parent.location &&
      /#\/apps\/([a-zA-Z0-9_-]+)/.exec(window.parent.location.hash);
    if (hashMatch && hashMatch[1]) {
      instanceName = hashMatch[1];
    }
    this.setInstanceNameInStore(instanceName);

    // Sync active user locale synchronously (en + de are pre-bundled in main.js)
    if (core && core.$i18n && core.$i18n.locale) {
      const normalized = String(core.$i18n.locale).substring(0, 2).toLowerCase();
      if (normalized === "en" || normalized === "de") {
        this.$i18n.locale = normalized;
      }
    }

    // Listen for route changes dispatched by NS8 core shell (`?page=...`)
    this.onChangeRoute = (e) => {
      const requestedPage = (e && e.detail) || "status";
      this.navigateInternalPage(requestedPage);
    };
    window.addEventListener("changeRoute", this.onChangeRoute, false);

    if (core && core.$root) {
      core.$root.$emit("configureKeyboardShortcuts", window);
      this.retrieveInstanceLabel();
    }

    // Honor initial `?page=` query parameter from NS8 parent URL (e.g. `?page=settings`)
    const queryParams = this.getQueryParamsForApp();
    const requestedPage = (queryParams && queryParams.page) || "status";
    if (requestedPage !== "status") {
      this.navigateInternalPage(requestedPage);
    }
  },
  beforeDestroy() {
    if (this.onChangeRoute) {
      window.removeEventListener("changeRoute", this.onChangeRoute, false);
    }
  },
  methods: {
    ...mapActions([
      "setInstanceNameInStore",
      "setInstanceLabelInStore",
      "setCoreInStore",
    ]),
    navigateInternalPage(page) {
      const clean = String(page || "status").replace(/^\/+/, "") || "status";
      const targetPath = "/" + clean;
      if (this.$route.path !== targetPath) {
        this.$router.replace(targetPath).catch(() => {});
      }
      this.$root.$emit("appNavigation");
    },
    async retrieveInstanceLabel() {
      if (!this.core || !this.instanceName) {
        return;
      }
      const taskAction = "get-name";
      const eventId = this.getUuid();
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.getInstanceLabelCompleted
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
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
      void res;
    },
    getInstanceLabelCompleted(taskContext, taskResult) {
      if (taskResult && taskResult.output && taskResult.output.name) {
        this.setInstanceLabelInStore(taskResult.output.name);
      }
    },
  },
};
</script>

<style lang="scss">
@import "~carbon-components/css/carbon-components.min.css";
@import "./styles/base";

.app-side-nav {
  top: 3rem;
}

.ns-app-main-content {
  margin-left: 16rem;
  margin-top: 0;
  background-color: #f4f4f4;
  min-height: 100vh;
}

@media (max-width: 1055px) {
  .app-side-nav {
    display: none;
  }

  .ns-app-main-content {
    margin-left: 0;
  }
}
</style>
