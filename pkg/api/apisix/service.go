package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type serviceClient struct {
	*resourceClient[types.Service]
}

func newService(c *Client) Service {
	cli := newResourceClient[types.Service](c, "apisix/admin/services")
	return &serviceClient{
		resourceClient: cli,
	}
}

func (u *serviceClient) Create(ctx context.Context, obj *types.Service) (*types.Service, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *serviceClient) Update(ctx context.Context, obj *types.Service) (*types.Service, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}
