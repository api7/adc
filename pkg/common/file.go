package common

import (
	"bufio"
	"io"
	"os"

	"github.com/fatih/color"
	"sigs.k8s.io/yaml"

	api7types "github.com/api7/adc/pkg/api/api7/types"
)

func GetContentFromFile(filename string) (*api7types.Configuration, error) {
	var content api7types.Configuration

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
