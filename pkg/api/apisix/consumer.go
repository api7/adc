package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type consumerClient struct {
	*resourceClient[types.Consumer]
}

func newConsumer(c *Client) Consumer {
	cli := newResourceClient[types.Consumer](c, "consumers")
	return &consumerClient{
		resourceClient: cli,
	}
}

func (u *consumerClient) Create(ctx context.Context, obj *types.Consumer) (*types.Consumer, error) {
	return u.resourceClient.Create(ctx, obj.Username, obj)
}

func (u *consumerClient) Update(ctx context.Context, obj *types.Consumer) (*types.Consumer, error) {
	return u.resourceClient.Update(ctx, obj.Username, obj)
}
