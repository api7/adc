package apisix

import (
	"context"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type Cluster interface {
	Route() Route
	Service() Service
	Consumer() Consumer
	SSL() SSL
	GlobalRule() GlobalRule
	PluginConfig() PluginConfig
}

type ResourceClient[T any] interface {
	Get(ctx context.Context, name string) (*T, error)
	List(ctx context.Context) ([]*T, error)
	Create(ctx context.Context, ups *T) (*T, error)
	Delete(ctx context.Context, name string) error
	Update(ctx context.Context, ups *T) (*T, error)
	Validate(ctx context.Context, resource *T) error
}

type Route interface {
	ResourceClient[types.Route]
}

type Service interface {
	ResourceClient[types.Service]
}

type Consumer interface {
	ResourceClient[types.Consumer]
}

type SSL interface {
	ResourceClient[types.SSL]
}

type GlobalRule interface {
	ResourceClient[types.GlobalRule]
}

type PluginConfig interface {
	ResourceClient[types.PluginConfig]
}
