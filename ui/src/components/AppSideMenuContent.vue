<template>
  <div class="app-side-menu-content">
    <div class="instance-name">
      <div v-if="instanceLabel">{{ instanceLabel }}</div>
      <div v-else-if="instanceName">{{ instanceName }}</div>
      <cv-skeleton-text v-else :width="instanceNameSkeletonWidth" />
    </div>

    <cv-side-nav-items>
      <cv-side-nav-link
        @click="navigateToPage('status')"
        :class="{ 'current-page': isLinkActive('status') }"
      >
        <template v-slot:nav-icon>
          <Activity20 />
        </template>
        <span>{{ $t("status.title") }}</span>
      </cv-side-nav-link>

      <cv-side-nav-link
        @click="navigateToPage('settings')"
        :class="{ 'current-page': isLinkActive('settings') }"
      >
        <template v-slot:nav-icon>
          <Settings20 />
        </template>
        <span>{{ $t("settings.title") }}</span>
      </cv-side-nav-link>

      <cv-side-nav-link
        @click="navigateToPage('about')"
        :class="{ 'current-page': isLinkActive('about') }"
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
import { QueryParamService, UtilService } from "@nethserver/ns8-ui-lib";
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
  mixins: [QueryParamService, UtilService],
  data() {
    return {
      instanceNameSkeletonWidth: "70%",
    };
  },
  computed: {
    ...mapState(["instanceName", "instanceLabel", "core"]),
  },
  created() {
    this.$root.$on("appNavigation", this.onAppNavigation);
  },
  beforeDestroy() {
    this.$root.$off("appNavigation", this.onAppNavigation);
  },
  methods: {
    onAppNavigation() {
      this.$forceUpdate();
    },
    isLinkActive(page) {
      if (this.core && typeof this.getPage === "function") {
        const currentPage = this.getPage();
        if (currentPage) {
          return currentPage === page;
        }
      }
      if (page === "status" && this.$route.path === "/") {
        return true;
      }
      return this.$route.path === `/${page}`;
    },
    navigateToPage(page) {
      if (this.core && this.core.$router) {
        this.goToAppPage(this.instanceName, page);
      } else if (this.$route.path !== `/${page}`) {
        this.$router.push(`/${page}`).catch(() => {});
      }
      this.$root.$emit("closeMobileSideMenu");
    },
  },
};
</script>

<style scoped lang="scss">
.instance-name span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
