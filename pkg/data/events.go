package data

import (
	"encoding/json"
	"fmt"
	"reflect"

	"github.com/hexops/gotextdiff"
	"github.com/hexops/gotextdiff/myers"
	"github.com/hexops/gotextdiff/span"
)

type ResourceType string

var (
	// ServiceResourceType is the resource type of service
	ServiceResourceType ResourceType = "service"
	// RouteResourceType is the resource type of route
	RouteResourceType ResourceType = "route"
)

const (
	CreateOption = iota
	DeleteOption
	UpdateOption
)

// Event is the event of adc
type Event struct {
	ResourceType ResourceType `json:"resource_type"`
	Option       int          `json:"option"`
	OldValue     interface{}  `json:"old_value"`
	Value        interface{}  `json:"value"`
}

func getName(field string, value interface{}) string {
	v := reflect.ValueOf(value)
	if v.Kind() == reflect.Ptr {
		v = v.Elem()
	}
	return v.FieldByName(field).String()
}

func (e *Event) Output() (string, error) {
	var output string
	switch e.Option {
	case CreateOption:
		output = fmt.Sprintf("creating %s: \"%s\"", e.ResourceType, getName("Name", e.Value))
	case DeleteOption:
		output = fmt.Sprintf("deleting %s: \"%s\"", e.ResourceType, getName("Name", e.OldValue))
	case UpdateOption:
		remote, err := json.MarshalIndent(e.OldValue, "", "\t")
		if err != nil {
			return "", err
		}
		remote = append(remote, '\n')

		local, err := json.MarshalIndent(e.Value, "", "\t")
		if err != nil {
			return "", err
		}
		local = append(local, '\n')

		edits := myers.ComputeEdits(span.URIFromPath("remote"), string(remote), string(local))
		diff := fmt.Sprint(gotextdiff.ToUnified("remote", "local", string(remote), edits))
		output = fmt.Sprintf("updating %s: \"%s\"\n%s", e.ResourceType, getName("Name", e.Value), diff)
	}

	return output, nil
}
