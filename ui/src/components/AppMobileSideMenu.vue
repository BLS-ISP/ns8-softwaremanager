<template>
  <transition name="slide-menu">
    <cv-side-nav
      v-if="isAppSideMenuShown"
      id="app-mobile-side-menu"
      :fixed="true"
      :expanded="true"
      class="app-side-nav"
    >
      <AppSideMenuContent />
    </cv-side-nav>
  </transition>
</template>

<script>
import { mapState } from "vuex";
import AppSideMenuContent from "./AppSideMenuContent.vue";

export default {
  name: "AppMobileSideMenu",
  components: {
    AppSideMenuContent,
  },
  computed: {
    ...mapState(["isAppSideMenuShown"]),
  },
};
</script>
