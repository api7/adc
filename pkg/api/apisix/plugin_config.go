package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type pluginConfigClient struct {
	*resourceClient[types.PluginConfig]
}

func newPluginConfig(c *Client) PluginConfig {
	cli := newResourceClient[types.PluginConfig](c, "plugin_configs")
	return &pluginConfigClient{
		resourceClient: cli,
	}
}

func (u *pluginConfigClient) Create(ctx context.Context, obj *types.PluginConfig) (*types.PluginConfig, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *pluginConfigClient) Update(ctx context.Context, obj *types.PluginConfig) (*types.PluginConfig, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}
