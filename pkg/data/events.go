package data

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/hexops/gotextdiff"
	"github.com/hexops/gotextdiff/myers"
	"github.com/hexops/gotextdiff/span"
	"github.com/pkg/errors"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
)

// ResourceType is the type of resource
type ResourceType string

var (
	// ServiceResourceType is the resource type of service
	ServiceResourceType ResourceType = "service"
	// RouteResourceType is the resource type of route
	RouteResourceType ResourceType = "route"
	// ConsumerResourceType is the resource type of consumer
	ConsumerResourceType ResourceType = "consumer"
	// SSLResourceType is the resource type of SSL
	SSLResourceType ResourceType = "ssl"
	// GlobalRuleResourceType is the resource type of global rule
	GlobalRuleResourceType ResourceType = "global_rule"
	// PluginConfigResourceType is the resource type of plugin config
	PluginConfigResourceType ResourceType = "plugin_config"
	// ConsumerGroupResourceType is the resource type of consumer group
	ConsumerGroupResourceType ResourceType = "consumer_group"
	// PluginMetadataResourceType is the resource type of consumer group
	PluginMetadataResourceType ResourceType = "plugin_metadata"
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

// Output returns the output of event,
// if the event is create, it will return the message of creating resource.
// if the event is update, it will return the diff of old value and new value.
// if the event is delete, it will return the message of deleting resource.
func (e *Event) Output() (string, error) {
	var output string
	switch e.Option {
	case CreateOption:
		output = fmt.Sprintf("creating %s: \"%s\"", e.ResourceType, apisix.GetResourceUniqueKey(e.Value))
	case DeleteOption:
		output = fmt.Sprintf("deleting %s: \"%s\"", e.ResourceType, apisix.GetResourceUniqueKey(e.OldValue))
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
		output = fmt.Sprintf("updating %s: \"%s\"\n%s", e.ResourceType, apisix.GetResourceUniqueKey(e.Value), diff)
	}

	return output, nil
}

func apply[T any](client apisix.ResourceClient[T], event *Event) error {
	var err error
	switch event.Option {
	case CreateOption:
		_, err = client.Create(context.Background(), event.Value.(*T))
	case DeleteOption:
		err = client.Delete(context.Background(), apisix.GetResourceUniqueKey(event.OldValue))
	case UpdateOption:
		_, err = client.Update(context.Background(), event.Value.(*T))
	}

	return errors.Wrap(err, "failed to apply "+string(event.ResourceType))
}

func applyService(cluster apisix.Cluster, event *Event) error {
	return apply[types.Service](cluster.Service(), event)
}

func applyRoute(cluster apisix.Cluster, event *Event) error {
	return apply[types.Route](cluster.Route(), event)
}

func applyConsumer(cluster apisix.Cluster, event *Event) error {
	return apply[types.Consumer](cluster.Consumer(), event)
}

func applySSL(cluster apisix.Cluster, event *Event) error {
	return apply[types.SSL](cluster.SSL(), event)
}

func applyGlobalRule(cluster apisix.Cluster, event *Event) error {
	return apply[types.GlobalRule](cluster.GlobalRule(), event)
}

func applyPluginConfig(cluster apisix.Cluster, event *Event) error {
	return apply[types.PluginConfig](cluster.PluginConfig(), event)
}

func applyConsumerGroup(cluster apisix.Cluster, event *Event) error {
	return apply[types.ConsumerGroup](cluster.ConsumerGroup(), event)
}

func applyPluginMetadata(cluster apisix.Cluster, event *Event) error {
	return apply[types.PluginMetadata](cluster.PluginMetadata(), event)
}

func (e *Event) Apply(cluster apisix.Cluster) error {
	switch e.ResourceType {
	case ServiceResourceType:
		return applyService(cluster, e)
	case RouteResourceType:
		return applyRoute(cluster, e)
	case ConsumerResourceType:
		return applyConsumer(cluster, e)
	case SSLResourceType:
		return applySSL(cluster, e)
	case GlobalRuleResourceType:
		return applyGlobalRule(cluster, e)
	case PluginConfigResourceType:
		return applyPluginConfig(cluster, e)
	case ConsumerGroupResourceType:
		return applyConsumerGroup(cluster, e)
	case PluginMetadataResourceType:
		return applyPluginMetadata(cluster, e)
	}

	return nil
}
