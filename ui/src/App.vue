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
import { loadLanguageAsync } from "./i18n";

export default {
  name: "App",
  components: {
    AppSideMenuContent,
    AppMobileSideMenu,
  },
  mixins: [QueryParamService, TaskService, UtilService],
  computed: {
    ...mapState(["instanceName", "core"]),
  },
  async created() {
    const core = window.parent && window.parent.core ? window.parent.core : null;
    this.setCoreInStore(core);

    let instanceName = "softwaremanager1";
    const hashMatch =
      window.parent &&
      window.parent.location &&
      window.parent.location.hash.match(/#\/apps\/([a-zA-Z0-9_-]+)/);
    if (hashMatch && hashMatch[1]) {
      instanceName = hashMatch[1];
    }
    this.setInstanceNameInStore(instanceName);

    // Sync active user locale with NS8 core (fallback en, preloaded en + de)
    if (core && core.$i18n && core.$i18n.locale) {
      await loadLanguageAsync(core.$i18n.locale, this.$i18n);
    }

    if (core && core.$root) {
      core.$root.$on("configureKeyboardShortcuts", this.configureKeyboardShortcuts);
      this.retrieveInstanceLabel();
    }
  },
  beforeDestroy() {
    if (this.core && this.core.$root) {
      this.core.$root.$off("configureKeyboardShortcuts", this.configureKeyboardShortcuts);
    }
  },
  methods: {
    ...mapActions([
      "setInstanceNameInStore",
      "setInstanceLabelInStore",
      "setCoreInStore",
    ]),
    configureKeyboardShortcuts() {
      // Handled by NS8 core shell
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
