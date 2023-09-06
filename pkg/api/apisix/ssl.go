package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type sslClient struct {
	*resourceClient[types.SSL]
}

func newSSL(c *Client) SSL {
	cli := newResourceClient[types.SSL](c, "ssls")
	return &sslClient{
		resourceClient: cli,
	}
}

func (u *sslClient) Create(ctx context.Context, obj *types.SSL) (*types.SSL, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *sslClient) Update(ctx context.Context, obj *types.SSL) (*types.SSL, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}
