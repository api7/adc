package common

import (
	"bufio"
	"io"
	"os"

	"github.com/fatih/color"
	"sigs.k8s.io/yaml"

	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/data"
)

func GetContentFromFile(filename string) (*data.Configuration, error) {
	var content data.Configuration

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

	return &content, nil
}

func GetContentFromRemote() (*data.Configuration, error) {
	return &data.Configuration{
		Services: []*data.Service{},
	}, nil
}

func LoadAPISIXConfiguration(filename string) (*types.Configuration, error) {
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

	return &content, nil
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
