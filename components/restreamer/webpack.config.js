const path = require('path');

const CopyPlugin = require('copy-webpack-plugin');
const MiniCssExtractPlugin = require('mini-css-extract-plugin');
const MinifyHtmlWebpackPlugin = require('minify-html-webpack-plugin');

const mode = process.env.NODE_ENV || 'development';
const is_prod = mode === 'production';

module.exports = {
  entry: {
    bundle: ['./src/main.js'],
  },
  resolve: {
    alias: {
      svelte: path.resolve('node_modules', 'svelte'),
    },
    extensions: ['.mjs', '.js', '.svelte'],
    mainFields: ['svelte', 'browser', 'module', 'main'],
  },
  output: {
    path: __dirname + '/public',
    filename: '[name].js',
    chunkFilename: '[name].[id].js',
  },
  module: {
    rules: [{
      test: /\.svelte$/,
      use: {
        loader: 'svelte-loader',
        options: {
          preprocess: require('svelte-preprocess')({}),
          emitCss: true,
          hotReload: true,
        },
      },
    }, {
      test: /\.css$/,
      use: [
        // 'mini-css-extract-plugin' doesn't support HMR.
        // Use 'style-loader' instead for development.
        is_prod ? MiniCssExtractPlugin.loader : 'style-loader',
        'css-loader',
      ],
    }, {
      test: /\.(graphql|gql)$/,
      exclude: /node_modules/,
      use: 'graphql-tag/loader',
    }],
  },
  mode,
  plugins: [
    new MiniCssExtractPlugin({
      filename: '[name].css'
    }),
    new CopyPlugin({
      patterns: [{from: 'static'}],
    }),
  ],
  devtool: is_prod ? false : 'source-map',
};

if (is_prod) {
  module.exports.plugins = (module.exports.plugins || []).concat([
    new MinifyHtmlWebpackPlugin({
      afterBuild: true,
      src: 'public',
      dest: 'public',
      ignoreFileNameRegex: /\.[^h.][^t.]?[^m.]?[^l.]?[^.]*$/,
      rules: {
        collapseBooleanAttributes: true,
        collapseWhitespace: true,
        removeAttributeQuotes: true,
        removeComments: true,
        minifyJS: true,
      }
    }),
  ]);
}
