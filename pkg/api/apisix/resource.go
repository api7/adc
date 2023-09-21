package apisix

import (
	"encoding/json"
	"errors"
	"fmt"
	"strconv"
	"strings"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type getResponse = item

type createResponse = item

type updateResponse = item

// listResponse is the v3 version unified LIST response mapping of APISIX.
type listResponse struct {
	Total IntOrString `json:"total"`
	List  items       `json:"list"`
}

// IntOrString processing number and string types, after json deserialization will output int
type IntOrString struct {
	IntValue int `json:"int_value"`
}

func (ios *IntOrString) UnmarshalJSON(p []byte) error {
	result := strings.Trim(string(p), "\"")
	count, err := strconv.Atoi(result)
	if err != nil {
		return err
	}
	ios.IntValue = count
	return nil
}

type items []item

// UnmarshalJSON implements json.Unmarshaler interface.
// lua-cjson doesn't distinguish empty array and table,
// and by default empty array will be encoded as '{}'.
// We have to maintain the compatibility.
func (items *items) UnmarshalJSON(p []byte) error {
	if p[0] == '{' {
		if len(p) != 2 {
			return errors.New("unexpected non-empty object")
		}
		return nil
	}
	var data []item
	if err := json.Unmarshal(p, &data); err != nil {
		return err
	}
	*items = data
	return nil
}

type item struct {
	Key   string          `json:"key"`
	Value json.RawMessage `json:"value"`
}

func unmarshalItem[T any](i *item) (*T, error) {
	list := strings.Split(i.Key, "/")
	if len(list) < 1 {
		return nil, fmt.Errorf("bad upstream config key: %s", i.Key)
	}

	var obj T
	if err := json.Unmarshal(i.Value, &obj); err != nil {
		return nil, err
	}

	// patch PluginMetadata since the response doesn't contain ID
	switch any(obj).(type) {
	case *types.PluginMetadata:
		any(obj).(*types.PluginMetadata).ID = list[len(list)-1]
	case types.PluginMetadata:
		any(&obj).(*types.PluginMetadata).ID = list[len(list)-1]
	}

	return &obj, nil
}
