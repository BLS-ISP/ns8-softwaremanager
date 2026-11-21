import Vue from "vue";
import CarbonComponentsVue from "@carbon/vue";
import ns8Lib, { Filters } from "@nethserver/ns8-ui-lib";
import VueI18n from "vue-i18n";
import VueDateFns from "vue-date-fns";
import axios from "axios";
import App from "./App.vue";
import router from "./router";
import store from "./store";
import enMessages from "../public/i18n/en/translation.json";
import deMessages from "../public/i18n/de/translation.json";

Vue.config.productionTip = false;
Vue.use(CarbonComponentsVue);
Vue.use(ns8Lib);
Vue.use(VueI18n);
Vue.use(VueDateFns);

Vue.prototype.axios = axios;

Object.keys(Filters).forEach((filterName) => {
  Vue.filter(filterName, Filters[filterName]);
});

const browserLang = (navigator.language || "en").split("-")[0];
const initialLocale = ["de", "en"].includes(browserLang) ? browserLang : "en";

const i18n = new VueI18n({
  locale: initialLocale,
  fallbackLocale: "en",
  silentTranslationWarn: true,
  messages: {
    de: deMessages,
    en: enMessages,
  },
});

new Vue({
  router,
  store,
  i18n,
  render: (h) => h(App),
}).$mount("#app");
