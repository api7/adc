package types

import (
	"encoding/json"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestPluginMetadataMarshal(t *testing.T) {
	metadata := &PluginMetadata{
		ID: "http-logger",
		Config: map[string]interface{}{
			"log_format": map[string]interface{}{
				"host":       "$host",
				"@timestamp": "$time_iso8601",
				"client_ip":  "$remote_addr",
			},
		},
	}

	out, err := json.Marshal(metadata)

	assert.Nil(t, err)
	assert.Equal(t, `{"id":"http-logger","log_format":{"@timestamp":"$time_iso8601","client_ip":"$remote_addr","host":"$host"}}`, string(out))

	var unmarshalled PluginMetadata
	err = json.Unmarshal(out, &unmarshalled)
	assert.Nil(t, err)

	assert.Equal(t, metadata.ID, unmarshalled.ID)
	expectedConf := metadata.Config["log_format"].(map[string]interface{})
	unmarshalledConf := unmarshalled.Config["log_format"].(map[string]interface{})
	assert.Equal(t, expectedConf["host"], unmarshalledConf["host"])
	assert.Equal(t, expectedConf["@timestamp"], unmarshalledConf["@timestamp"])
	assert.Equal(t, expectedConf["client_ip"], unmarshalledConf["client_ip"])
}
