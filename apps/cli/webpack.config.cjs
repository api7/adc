const { composePlugins, withNx } = require('@nx/webpack');
const { DefinePlugin, optimize } = require('webpack');
const TerserPlugin = require("terser-webpack-plugin");

// Nx plugins for webpack.
module.exports = composePlugins(
  withNx({
    target: 'node',
  }),
  (config) => {
    config.devtool = 'inline-source-map';
    config.output.library = {
      type: 'module',
    }
    config.output.devtoolModuleFilenameTemplate = "[absolute-resource-path]";
    config.externals = {
      '*': false
    };
    config.plugins.push(...[
      new DefinePlugin({
        'process.env.NODE_ENV': `'${process.env.NODE_ENV ?? 'development'}'`
      }),
      new optimize.LimitChunkCountPlugin({
        maxChunks: 1
      }),
    ]);
    config.optimization = {
      minimize: true,
      minimizer: [new TerserPlugin({
        parallel: true,
        extractComments: false,
        terserOptions: {
          compress: true,
          mangle: false,
          sourceMap: true,
        },
      })],
    };
    config.experiments.outputModule = true;
    return config;
  }
);
