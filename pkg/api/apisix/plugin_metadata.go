package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type pluginMetadataClient struct {
	*resourceClient[types.PluginMetadata]
}

func newPluginMetadata(c *Client) PluginMetadata {
	cli := newResourceClient[types.PluginMetadata](c, "plugin_metadata")
	return &pluginMetadataClient{
		resourceClient: cli,
	}
}

func (u *pluginMetadataClient) Create(ctx context.Context, obj *types.PluginMetadata) (*types.PluginMetadata, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *pluginMetadataClient) Update(ctx context.Context, obj *types.PluginMetadata) (*types.PluginMetadata, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}
