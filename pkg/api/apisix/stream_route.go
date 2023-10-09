package apisix

import (
	"context"
	"strings"

	"github.com/pkg/errors"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type streamRouteClient struct {
	*resourceClient[types.StreamRoute]
}

func newStreamRoute(c *Client) StreamRoute {
	cli := newResourceClient[types.StreamRoute](c, "stream_routes")
	return &streamRouteClient{
		resourceClient: cli,
	}
}

func (u *streamRouteClient) Create(ctx context.Context, obj *types.StreamRoute) (*types.StreamRoute, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *streamRouteClient) Update(ctx context.Context, obj *types.StreamRoute) (*types.StreamRoute, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}

func (u *streamRouteClient) List(ctx context.Context) ([]*types.StreamRoute, error) {
	items, err := u.resourceClient.List(ctx)
	if err != nil {
		if !strings.Contains(err.Error(), "stream mode is disabled") {
			return nil, errors.Wrap(err, "failed to list stream_routes")
		}
	}
	return items, nil
}
