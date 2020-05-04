const path = require("path")

const CopyPlugin = require("copy-webpack-plugin")
const CssNanoPlugin = require("cssnano-webpack-plugin");
const HtmlWebpackPlugin = require("html-webpack-plugin")
const MiniCssExtractPlugin = require("mini-css-extract-plugin")
const PostStylus = require("poststylus")
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin")
const webpack = require("webpack")

const dist = path.resolve(__dirname, "dist/")

const isProd = (process.env.NODE_ENV === "production")

module.exports = {
  mode: isProd ? "production" : "development",
  entry: {
    main: path.resolve(__dirname, "src/main.js"),
  },
  output: {
    path: dist,
    filename: "[name].js"
  },
  devServer: {
    contentBase: dist,
  },
  module: {
    rules: [{
      test: /\.styl$/,
      use: [
        MiniCssExtractPlugin.loader,
        'css-loader',
        'stylus-loader',
      ],
    }],
  },
  plugins: [
    //new CopyPlugin([
    //  path.resolve(__dirname, "static")
    //]),

    new WasmPackPlugin({crateDirectory: __dirname}),

    new webpack.LoaderOptionsPlugin({
      options: {
        stylus: {
          use: [PostStylus(["autoprefixer", "rucksack-css"])],
        },
      },
    }),
    new MiniCssExtractPlugin({
      filename: "[name].css",
    }),

    new HtmlWebpackPlugin({
      template: 'src/index.html',
      hash: true,
    }),
  ],
  optimization: {
    minimizer: [new CssNanoPlugin()],
  },
}
