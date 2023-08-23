/*
Copyright © 2023 API7.ai
*/
package config

import "github.com/api7/adc/pkg/api/apisix"

type ClientConfig struct {
	Server string
	Token  string

	APISIXCluster apisix.Cluster
}
