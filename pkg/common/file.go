package common

import (
	"bufio"
	"io"
	"os"

	"github.com/fatih/color"
	"sigs.k8s.io/yaml"

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
