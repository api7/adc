package openapi2apisix

import (
	"context"
	"fmt"
	"net/url"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/getkin/kin-openapi/openapi3"
	"github.com/mozillazg/go-slugify"
	"github.com/pkg/errors"

	apitypes "github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

var (
	_pathVariableRegex = regexp.MustCompile(`{[^}]+}`)
)

func convertPathVariables(path string) string {
	return _pathVariableRegex.ReplaceAllString(path, "*")
}

// Slugify converts a name to a valid name by removing and replacing unallowed characters
// and sanitizing non-latin characters. Multiple inputs will be concatenated using '_'.
func Slugify(name ...string) string {
	for i, elem := range name {
		name[i] = slugify.Slugify(elem)
	}

	return strings.Join(name, "_")
}

// Convert converts OAS to API Service and Routes
func Convert(ctx context.Context, oas []byte) (*apitypes.Configuration, error) {

	result := apitypes.Configuration{}

	doc, err := OAS(oas).LoadOpenAPI(ctx)
	if err != nil {
		return nil, errors.Wrap(LoadOpenAPIError, err.Error())
	}

	serverUris, varUris, err := parseServerUris(&doc.Servers)
	if err != nil {
		return nil, errors.Wrap(LoadOpenAPIError, err.Error())
	}

	ups := &apitypes.Upstream{
		Nodes: []apitypes.UpstreamNode{
			apitypes.UpstreamNode{},
		},
	}

	if len(serverUris)+len(varUris) > 0 {
		ups = createUpstream(serverUris, varUris)
	}

	// handle services
	upsName := Slugify("Upstream for ", doc.Info.Title)
	ups.Name = upsName
	ups.ID = common.GenID(ups.Name)

	result.Services = []*apitypes.Service{
		{
			ID:          common.GenID(doc.Info.Title),
			Name:        doc.Info.Title,
			Description: doc.Info.Description,
			//Labels:      getLabelsByTags(doc.Tags),
			Upstream: ups,
		},
	}

	// handle routes
	routes := createRoutes(doc, result.Services[0].ID)
	result.Routes = routes

	return &result, nil
}

func createRoutes(doc *openapi3.T, serviceID string) []*apitypes.Route {
	// create a sorted array of paths, to be deterministic in our output order
	sortedPaths := make([]string, len(doc.Paths))
	i := 0
	for path := range doc.Paths {
		sortedPaths[i] = path
		i++
	}
	sort.Strings(sortedPaths)

	var routes []*apitypes.Route
	for _, path := range sortedPaths {
		pathItem := doc.Paths[path]
		operations := pathItem.Operations()
		sortedMethods := make([]string, len(operations))
		i := 0
		for method := range operations {
			sortedMethods[i] = method
			i++
		}
		sort.Strings(sortedMethods)
		for _, method := range sortedMethods {
			operation := operations[method]
			// route name
			routeName := operation.OperationID
			if routeName == "" {
				routeName = Slugify(doc.Info.Title, method, path)
			}
			routeDescription := operation.Summary
			if routeDescription == "" {
				routeDescription = operation.Description
			}
			routePath := convertPathVariables(path)
			// var tags []string
			// for _, tag := range operation.Tags {
			// tags = append(tags, Slugify(tag))
			// }
			route := apitypes.Route{
				ID:          common.GenID(routeName),
				Name:        routeName,
				Description: routeDescription,
				// Labels:      tags,
				Methods:   []string{method},
				Uris:      []string{routePath},
				ServiceID: serviceID,
			}
			routes = append(routes, &route)
		}
	}
	return routes
}

//nolint:unused
func getLabelsByTags(tags openapi3.Tags) apitypes.Labels {
	labels := apitypes.Labels{}
	for i, tag := range tags {
		labels[strconv.Itoa(i)] = Slugify(tag.Name)
	}

	return labels
}

//nolint:unused
func getServiceProtocols(targets []*url.URL) []string {
	protocols := map[string]struct{}{}
	for _, target := range targets {
		protocols[strings.ToUpper(target.Scheme)] = struct{}{}
	}
	var result []string
	for protocol := range protocols {
		result = append(result, protocol)
	}

	return result
}

func createUpstream(targets []*url.URL, varUris []string) *apitypes.Upstream {
	var upstreamNodes apitypes.UpstreamNodes
	for _, target := range targets {
		upstreamNodes = append(upstreamNodes, apitypes.UpstreamNode{
			Host:   target.Hostname(),
			Port:   getURLPort(target),
			Weight: 100,
		})
	}
	for _, uri := range varUris {
		host := strings.ReplaceAll(uri, "https://", "")
		host = strings.ReplaceAll(host, "http://", "")
		upstreamNodes = append(upstreamNodes, apitypes.UpstreamNode{
			Host:   host,
			Port:   getURLPortFromString(uri),
			Weight: 100,
		})
	}
	upstream := apitypes.Upstream{
		Scheme: targets[0].Scheme,
		Nodes:  upstreamNodes,
		Timeout: &apitypes.UpstreamTimeout{
			Connect: 60,
			Send:    60,
			Read:    60,
		},
		PassHost: apitypes.UpstreamPassHostPass,
	}

	return &upstream
}

// parseServerUris parses the server uri's after rendering the template variables.
// result will always have at least 1 entry, but not necessarily a hostname/port/scheme
func parseServerUris(servers *openapi3.Servers) ([]*url.URL, []string, error) {
	var (
		targets []*url.URL
		ignored []string
	)

	if servers == nil || len(*servers) == 0 {
		uriObject, _ := url.ParseRequestURI("/") // path '/' is the default for empty server blocks
		targets = make([]*url.URL, 1)
		targets[0] = uriObject
	} else {
		targets = []*url.URL{}

		for _, server := range *servers {
			uriString := server.URL
			for name, svar := range server.Variables {
				uriString = strings.ReplaceAll(uriString, "{"+name+"}", svar.Default)
			}
			if strings.Contains(uriString, "{") && strings.Contains(uriString, "}") {
				//color.Yellow("Server url with variable " + uriString + " ignored")
				ignored = append(ignored, uriString)
				continue
			}

			uriObject, err := url.ParseRequestURI(uriString)
			if err != nil {
				return targets, ignored, fmt.Errorf("failed to parse uri '%s'; %w", uriString, err)
			}

			if uriObject.Path == "" {
				uriObject.Path = "/" // path '/' is the default
			}

			targets = append(targets, uriObject)
			//targets[i] = uriObject
		}
	}

	return targets, ignored, nil
}

func getURLPort(u *url.URL) int {
	var port int
	if u.Port() != "" {
		port, _ = strconv.Atoi(u.Port())
	} else {
		if u.Scheme == "https" {
			port = 443
		} else {
			port = 80
		}
	}

	return port
}

func getURLPortFromString(s string) int {
	port := 80
	if strings.HasPrefix(s, "https://") {
		port = 443
	}

	s = strings.ReplaceAll(s, "{", "")
	s = strings.ReplaceAll(s, "}", "")
	uri, err := url.ParseRequestURI(s)
	if err != nil {
		return port
	}
	return getURLPort(uri)
}
