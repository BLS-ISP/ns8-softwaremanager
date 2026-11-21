<template>
  <div class="app-side-menu-content">
    <div class="instance-name">
      <div v-if="instanceLabel">{{ instanceLabel }}</div>
      <div v-else-if="instanceName">{{ instanceName }}</div>
      <cv-skeleton-text v-else :heading="true" width="8rem" />
    </div>

    <cv-side-nav-items>
      <cv-side-nav-link
        @click="goToAppPage(instanceName + '/status')"
        :active="isLinkActive('status')"
      >
        <template v-slot:nav-icon>
          <Activity20 />
        </template>
        <span>{{ $t("status.title") }}</span>
      </cv-side-nav-link>

      <cv-side-nav-link
        @click="goToAppPage(instanceName + '/settings')"
        :active="isLinkActive('settings')"
      >
        <template v-slot:nav-icon>
          <Settings20 />
        </template>
        <span>{{ $t("settings.title") }}</span>
      </cv-side-nav-link>

      <cv-side-nav-link
        @click="goToAppPage(instanceName + '/about')"
        :active="isLinkActive('about')"
      >
        <template v-slot:nav-icon>
          <Information20 />
        </template>
        <span>{{ $t("about.title") }}</span>
      </cv-side-nav-link>
    </cv-side-nav-items>
  </div>
</template>

<script>
import { mapState } from "vuex";
import { UtilService } from "@nethserver/ns8-ui-lib";
import Activity20 from "@carbon/icons-vue/es/activity/20";
import Settings20 from "@carbon/icons-vue/es/settings/20";
import Information20 from "@carbon/icons-vue/es/information/20";

export default {
  name: "AppSideMenuContent",
  components: {
    Activity20,
    Settings20,
    Information20,
  },
  mixins: [UtilService],
  computed: {
    ...mapState(["instanceName", "instanceLabel", "core"]),
  },
  methods: {
    isLinkActive(page) {
      return this.$route.path.includes(`/${page}`);
    },
    goToAppPage(path) {
      const targetPage = path.split("/")[1] || "status";
      if (this.core && this.core.$router) {
        this.core.$router.push(`/apps/${path}`);
      } else if (this.$route.path !== `/${targetPage}`) {
        this.$router.push(`/${targetPage}`);
      }
      this.$store.dispatch("setAppSideMenuShownInStore", false);
    },
  },
};
</script>

<style scoped lang="scss">
.instance-name {
  padding: 1rem 1rem 0.75rem;
  font-weight: 600;
  font-size: 0.95rem;
  color: #f4f4f4;
  border-bottom: 1px solid #393939;
}
</style>
