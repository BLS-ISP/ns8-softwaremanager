const path = require("path");
const { defineConfig } = require("@vue/cli-service");

module.exports = defineConfig({
  publicPath: "./",
  productionSourceMap: false,
  css: {
    loaderOptions: {
      sass: {
        sassOptions: {
          quietDeps: true,
        },
      },
    },
  },
  configureWebpack: {
    resolve: {
      alias: {
        crypto: path.resolve(__dirname, "src/shims/crypto.js"),
      },
    },
  },
  chainWebpack: (config) => {
    config.performance.hints(false);
  },
});
