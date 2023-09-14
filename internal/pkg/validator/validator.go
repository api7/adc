package validator

import (
	"context"
	"errors"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

type Validator struct {
	localConfig *types.Configuration
	cluster     apisix.Cluster
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

	common.NormalizeConfiguration(v.localConfig)

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

	for _, consumer := range v.localConfig.Consumers {
		consumer := consumer
		err := v.cluster.Consumer().Validate(context.Background(), consumer)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	for _, ssl := range v.localConfig.SSLs {
		ssl := ssl
		err := v.cluster.SSL().Validate(context.Background(), ssl)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	for _, globalRule := range v.localConfig.GlobalRules {
		globalRule := globalRule
		err := v.cluster.GlobalRule().Validate(context.Background(), globalRule)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	for _, pluginConfig := range v.localConfig.PluginConfigs {
		pluginConfig := pluginConfig
		err := v.cluster.PluginConfig().Validate(context.Background(), pluginConfig)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	for _, consumerGroup := range v.localConfig.ConsumerGroups {
		consumerGroup := consumerGroup
		err := v.cluster.ConsumerGroup().Validate(context.Background(), consumerGroup)
		if err != nil {
			allErr = append(allErr, err)
		}
	}

	// TODO: enable this when APISIX supports
	//for _, pluginMetadata := range v.localConfig.PluginMetadatas {
	//	pluginMetadata := pluginMetadata
	//	err := v.cluster.PluginMetadata().Validate(context.Background(), pluginMetadata)
	//	if err != nil {
	//		allErr = append(allErr, err)
	//	}
	//}

	return allErr
}
