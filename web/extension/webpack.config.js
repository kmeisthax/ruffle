/* eslint-env node */

const { CleanWebpackPlugin } = require("clean-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const path = require("path");

module.exports = (env, argv) => {
    let mode = "production";
    if (argv && argv.mode) {
        mode = argv.mode;
    }

    console.log(`Building ${mode}...`);

    return {
        entry: {
          "ruffle": path.resolve(__dirname, "js/index.js"),
          "popup": path.resolve(__dirname, "js/popup.js")
        },
        output: {
            path: path.resolve(__dirname, "build/dist"),
            filename: "[name].js",
            chunkFilename: "core.ruffle.js",
            jsonpFunction: "RufflePlayerExtensionLoader",
        },
        mode: mode,
        plugins: [
            new CleanWebpackPlugin(),
            new WasmPackPlugin({
                crateDirectory: path.resolve(__dirname, ".."),
                outName: "ruffle",
                forceMode: mode,
            }),
        ],
    };
};
