<template>
  <transition name="slide-menu">
    <div
      v-if="isMenuShown"
      class="
        mobile-side-menu
        cv-side-nav
        bx--side-nav bx--side-nav__navigation
        bx--side-nav--expanded
        app-side-nav
      "
    >
      <AppSideMenuContent />
    </div>
  </transition>
</template>

<script>
import AppSideMenuContent from "./AppSideMenuContent.vue";

export default {
  name: "AppMobileSideMenu",
  components: {
    AppSideMenuContent,
  },
  data() {
    return {
      isMenuShown: false,
    };
  },
  created() {
    this.$root.$on("toggleMobileSideMenu", this.toggleMobileSideMenu);
    this.$root.$on("closeMobileSideMenu", this.closeMobileSideMenu);
  },
  beforeDestroy() {
    this.$root.$off("toggleMobileSideMenu", this.toggleMobileSideMenu);
    this.$root.$off("closeMobileSideMenu", this.closeMobileSideMenu);
  },
  methods: {
    toggleMobileSideMenu() {
      this.isMenuShown = !this.isMenuShown;
    },
    closeMobileSideMenu() {
      this.isMenuShown = false;
    },
  },
};
</script>

<style scoped lang="scss">
@import "../styles/base";

.mobile-side-menu {
  width: $side-menu-width;
  height: calc(100vh - 3rem);
  position: fixed;
  top: 3rem;
  left: 0;
  z-index: 10000;
  overflow: auto;
}

.slide-menu-enter-active,
.slide-menu-leave-active {
  transition: all 0.3s ease;
}

.slide-menu-enter,
.slide-menu-leave-to {
  transform: translateX(-$side-menu-width);
}
</style>
