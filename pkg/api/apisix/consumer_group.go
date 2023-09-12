package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type consumerGroupClient struct {
	*resourceClient[types.ConsumerGroup]
}

func newConsumerGroup(c *Client) ConsumerGroup {
	cli := newResourceClient[types.ConsumerGroup](c, "consumer_groups")
	return &consumerGroupClient{
		resourceClient: cli,
	}
}

func (u *consumerGroupClient) Create(ctx context.Context, obj *types.ConsumerGroup) (*types.ConsumerGroup, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *consumerGroupClient) Update(ctx context.Context, obj *types.ConsumerGroup) (*types.ConsumerGroup, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}
