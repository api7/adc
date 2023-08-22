package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type routeClient struct {
	*resourceClient[types.Route]
}

func newRoute(c *Client) Route {
	cli := newResourceClient[types.Route](c, "routes")
	return &routeClient{
		resourceClient: cli,
	}
}

func (u *routeClient) Create(ctx context.Context, obj *types.Route) (*types.Route, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *routeClient) Update(ctx context.Context, obj *types.Route) (*types.Route, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}
