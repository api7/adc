package data

import (
	"encoding/json"
	"fmt"
	"reflect"

	"github.com/hexops/gotextdiff"
	"github.com/hexops/gotextdiff/myers"
	"github.com/hexops/gotextdiff/span"
)

// ResourceType is the type of resource
type ResourceType string

var (
	// ServiceResourceType is the resource type of service
	ServiceResourceType ResourceType = "service"
	// RouteResourceType is the resource type of route
	RouteResourceType ResourceType = "route"
)

const (
	// CreateOption is the option of create
	CreateOption = iota
	// DeleteOption is the option of delete
	DeleteOption
	// UpdateOption is the option of update
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

// Output returns the output of event,
// if the event is create, it will return the message of creating resource.
// if the event is update, it will return the diff of old value and new value.
// if the event is delete, it will return the message of deleting resource.
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
