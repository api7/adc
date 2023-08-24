package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type Cluster interface {
	Route() Route
	Service() Service
}

type Route interface {
	Get(ctx context.Context, name string) (*types.Route, error)
	List(ctx context.Context) ([]*types.Route, error)
	Create(ctx context.Context, ups *types.Route) (*types.Route, error)
	Delete(ctx context.Context, name string) error
	Update(ctx context.Context, ups *types.Route) (*types.Route, error)
	Validate(ctx context.Context, resource *types.Route) error
}

type Service interface {
	Get(ctx context.Context, name string) (*types.Service, error)
	List(ctx context.Context) ([]*types.Service, error)
	Create(ctx context.Context, ups *types.Service) (*types.Service, error)
	Delete(ctx context.Context, name string) error
	Update(ctx context.Context, ups *types.Service) (*types.Service, error)
	Validate(ctx context.Context, resource *types.Service) error
}
