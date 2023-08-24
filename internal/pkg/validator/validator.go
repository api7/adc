package validator

import (
	"context"
	"errors"

	"github.com/fatih/color"
	"sigs.k8s.io/yaml"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/data"
)

type Validator struct {
	localConfig *types.Configuration
	cluster     apisix.Cluster

	evenChan *data.Event
}

func NewValidator(local *types.Configuration, cluster apisix.Cluster) (*Validator, error) {
	return &Validator{
		localConfig: local,
		cluster:     cluster,
	}, nil
}

type ErrorsWrapper struct {
	Errors []error
}

func (v ErrorsWrapper) Error() string {
	var errStr string
	for _, e := range v.Errors {
		errStr += e.Error()
		if !errors.Is(e, v.Errors[len(v.Errors)-1]) {
			errStr += "\n"
		}
	}
	return errStr
}

func (v *Validator) Validate() []error {
	allErr := []error{}

	for _, service := range v.localConfig.Services {
		service := service
		if service.ID == "" {
			service.ID = service.Name
		}
		if service.Upstream.ID == "" {
			service.Upstream.ID = service.Upstream.Name
		}
		err := v.cluster.Service().Validate(context.Background(), service)
		if err != nil {
			b, _ := yaml.Marshal(service)
			color.Red(string(b))
			allErr = append(allErr, err)
		}
	}

	for _, route := range v.localConfig.Routes {
		route := route
		if route.ID == "" {
			route.ID = route.Name
		}
		err := v.cluster.Route().Validate(context.Background(), route)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	return allErr
}
