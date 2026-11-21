import Vue from "vue";
import Vuex from "vuex";

Vue.use(Vuex);

export default new Vuex.Store({
  state: {
    instanceName: "",
    instanceLabel: "",
    core: null,
    appName: "SoftwareManager",
    isAppSideMenuShown: false,
    configured: false,
    appUrl: "",
  },
  getters: {
    instanceName: (state) => state.instanceName,
    instanceLabel: (state) => state.instanceLabel,
    core: (state) => state.core,
  },
  mutations: {
    setInstanceName(state, instanceName) {
      state.instanceName = instanceName;
    },
    setInstanceLabel(state, instanceLabel) {
      state.instanceLabel = instanceLabel;
    },
    setCore(state, core) {
      state.core = core;
    },
    setAppSideMenuShown(state, value) {
      state.isAppSideMenuShown = value;
    },
    setConfigured(state, configured) {
      state.configured = Boolean(configured);
    },
    setAppUrl(state, appUrl) {
      state.appUrl = appUrl || "";
    },
  },
  actions: {
    setInstanceNameInStore({ commit }, instanceName) {
      commit("setInstanceName", instanceName);
    },
    setInstanceLabelInStore({ commit }, instanceLabel) {
      commit("setInstanceLabel", instanceLabel);
    },
    setCoreInStore({ commit }, core) {
      commit("setCore", core);
    },
    setAppSideMenuShownInStore({ commit }, value) {
      commit("setAppSideMenuShown", value);
    },
    setConfiguredInStore({ commit }, configured) {
      commit("setConfigured", configured);
    },
    setAppUrlInStore({ commit }, appUrl) {
      commit("setAppUrl", appUrl);
    },
  },
});
