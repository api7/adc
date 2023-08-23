package validator

import (
	"context"
	"errors"

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
		err := v.cluster.Service().Validate(context.Background(), service)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	for _, route := range v.localConfig.Routes {
		route := route
		err := v.cluster.Route().Validate(context.Background(), route)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	return allErr
}
