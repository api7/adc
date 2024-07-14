const { composePlugins, withNx } = require('@nx/webpack');
const { DefinePlugin, optimize } = require('webpack');

// Nx plugins for webpack.
module.exports = composePlugins(
  withNx({
    target: 'node',
  }),
  (config) => {
    config.externals = {
      '*': false
    }
    config.plugins.push(...[
      new DefinePlugin({
        'process.env.NODE_ENV': `'${process.env.NODE_ENV ?? 'development'}'`
      }),
      new optimize.LimitChunkCountPlugin({
        maxChunks: 1
      }),
    ])
    return config;
  }
);
