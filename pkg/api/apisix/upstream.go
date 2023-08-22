package apisix

import (
	"context"
	"github.com/api7/adc/pkg/api/apisix/types"
)

type upstreamClient struct {
	*resourceClient[types.Upstream]
}

func newUpstream(c *Client) Upstream {
	cli := newResourceClient[types.Upstream](c, "upstreams")
	return &upstreamClient{
		resourceClient: cli,
	}
}

func (u *upstreamClient) Create(ctx context.Context, obj *types.Upstream) (*types.Upstream, error) {
	return u.resourceClient.Create(ctx, obj.Name, obj)
}

func (u *upstreamClient) Update(ctx context.Context, obj *types.Upstream) (*types.Upstream, error) {
	return u.resourceClient.Update(ctx, obj.Name, obj)
}
