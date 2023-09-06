package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type globalRuleClient struct {
	*resourceClient[types.GlobalRule]
}

func newGlobalRule(c *Client) GlobalRule {
	cli := newResourceClient[types.GlobalRule](c, "global_rules")
	return &globalRuleClient{
		resourceClient: cli,
	}
}

func (u *globalRuleClient) Create(ctx context.Context, obj *types.GlobalRule) (*types.GlobalRule, error) {
	return u.resourceClient.Create(ctx, obj.ID, obj)
}

func (u *globalRuleClient) Update(ctx context.Context, obj *types.GlobalRule) (*types.GlobalRule, error) {
	return u.resourceClient.Update(ctx, obj.ID, obj)
}
