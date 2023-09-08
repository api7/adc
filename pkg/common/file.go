package common

import (
	"bufio"
	"context"
	"io"
	"os"

	"github.com/fatih/color"
	"sigs.k8s.io/yaml"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
)

func NormalizeConfiguration(content *types.Configuration) {
	for _, route := range content.Routes {
		if route.ID == "" {
			route.ID = route.Name
		}
	}

	for _, service := range content.Services {
		if service.ID == "" {
			service.ID = service.Name
		}
		if service.Upstream.ID == "" {
			service.Upstream.ID = service.Upstream.Name
		}
	}

	for _, pluginConfig := range content.PluginConfigs {
		if pluginConfig.ID == "" {
			pluginConfig.ID = pluginConfig.Name
		}
	}
}

func GetContentFromFile(filename string) (*types.Configuration, error) {
	var content types.Configuration

	f, err := os.Open(filename)
	if err != nil {
		color.Red("Open file %s failed: %s", filename, err)
		return nil, err
	}
	defer f.Close()

	reader := bufio.NewReader(f)
	fileContent, err := io.ReadAll(reader)
	if err != nil {
		color.Red("Read file %s failed: %s", filename, err)
		return nil, err
	}

	// I should use YAML unmarshal the fileContent to a Configuration struct
	err = yaml.Unmarshal(fileContent, &content)
	if err != nil {
		color.Red("Unmarshal file %s failed: %s", filename, err)
		return nil, err
	}

	NormalizeConfiguration(&content)

	return &content, nil
}

func GetContentFromRemote(cluster apisix.Cluster) (*types.Configuration, error) {
	svcs, err := cluster.Service().List(context.Background())
	if err != nil {
		return nil, err
	}

	routes, err := cluster.Route().List(context.Background())
	if err != nil {
		return nil, err
	}

	consumers, err := cluster.Consumer().List(context.Background())
	if err != nil {
		return nil, err
	}

	ssls, err := cluster.SSL().List(context.Background())
	if err != nil {
		return nil, err
	}

	return &types.Configuration{
		Routes:    routes,
		Services:  svcs,
		Consumers: consumers,
		SSLs:      ssls,
	}, nil
}

func SaveAPISIXConfiguration(path string, conf *types.Configuration) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}

	defer f.Close()

	data, err := yaml.Marshal(conf)
	if err != nil {
		color.Red(err.Error())
		return err
	}

	_, err = f.Write(data)
	if err != nil {
		return err
	}

	err = f.Sync()
	if err != nil {
		return err
	}

	return nil
}
