package data

import (
	"fmt"
)

// StringArray is enhanced version of pq.StringArray that can be handled nil value automatically.
type StringArray []string

// Plugins contains a collect of polices like CORS.
type Plugins map[string]interface{}

// Service is the abstraction of a backend service on API gateway.
type Service struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	// Labels are used for resource classification and indexing
	Labels StringArray `json:"labels,omitempty"`
	// Protocols indicates the protocols that the service supports
	Protocols []string `json:"protocols,omitempty"`
	// The listening path prefix for this service.
	PathPrefix string `json:"path_prefix,omitempty"`
	// HTTP hosts for this service.
	Hosts []string `json:"hosts"`
	// Plugin settings on Service level
	Plugins Plugins `json:"plugins,omitempty"`
	// Upstream settings for the Service.
	Upstreams []Upstream `json:"upstreams"`
	// UpstreamInUse specifies the name of upstream that is in use
	UpstreamInUse string `json:"upstream_in_use,omitempty"`
}

// Upstream is the definition of the upstream on Service.
type Upstream struct {
	// Name is the upstream name. It should be unique among all upstreams
	// in the same service.
	Name string `json:"name"`
	// The scheme to communicate with the upstream
	Scheme string `json:"scheme"`
	// LBType is the load balancing strategy of the upstream
	LBType string `json:"lb_type,omitempty"`
	// HashKey is the hash key used to balance the upstream
	HashKey string `json:"hash_key,omitempty"`
	// The upstream endpoints
	Targets []UpstreamTarget `json:"targets,omitempty"`
	// Retries is sets the number of retries while passing the request to Upstream using the underlying Nginx mechanism.
	Retries *int `json:"retries,omitempty"`
	// Timeout is sets the timeout for connecting to, and sending and receiving messages to and from the Upstream
	Timeout *UpstreamTimeout `json:"timeout,omitempty"`
	// UpstreamHostMode configures the host header when the request is forwarded to the upstream
	UpstreamHostMode string `json:"upstream_host_mode,omitempty"`
	// UpstreamHost specifies the host of the Upstream request, this is only valid if the upstream_host_mode is set to rewrite
	UpstreamHost string `json:"upstream_host,omitempty"`
	//Checks the data of health check
	Checks *Checks `json:"checks,omitempty"`
}

// Route is the API endpoint which is exposed to the outside world by its service.
type Route struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	// Labels are used for resource classification and indexing
	Labels  StringArray `json:"labels,omitempty"`
	Methods []string    `json:"methods"`
	// Paths is the route paths to match this route.
	Paths []string `json:"paths"`
	// StripePathPrefix indicates whether to strip the path prefix defined on service.
	StripPathPrefix bool `json:"strip_path_prefix"`
	// Plugin settings on Service level
	Plugins Plugins `json:"plugins,omitempty"`
	// ServiceID is id of service that current route belong with.
	ServiceID string `json:"service_id"`
	// EnableWebSocket indicates whether this route should support websocket upgrade.
	EnableWebSocket bool `json:"enable_websocket"`
}

// UpstreamTarget is the definition for an upstream endpoint.
type UpstreamTarget struct {
	Host   string `json:"host"`
	Port   int    `json:"port"`
	Weight int    `json:"weight"`
}

// UpstreamTimeout is the timeout for connecting to, and sending and receiving messages to and from the Upstream, value in seconds.
type UpstreamTimeout struct {
	Connect int `json:"connect,omitempty"`
	Send    int `json:"send,omitempty"`
	Read    int `json:"read,omitempty"`
}

const (
	HealthCheckTypeTCP   = "tcp"
	HealthCheckTypeHTTP  = "http"
	HealthCheckTypeHTTPS = "https"
)

const (
	// ConsistentHashLoadBalancer means the consistent hash load balancer
	ConsistentHashLoadBalancer = "consistent_hash"
	// RoundRobinLoadBalancer means the roundrobin load balancer
	RoundRobinLoadBalancer = "roundrobin"
	// EWMALoadBalancer means the ewma load balancer
	EWMALoadBalancer = "ewma"
	// LeastConnLoadBalancer means the least connection load balancer
	LeastConnLoadBalancer = "least_conn"
)

const (
	// UpstreamReserveHost reserve host from the client's request as upstream request host.
	UpstreamReserveHost = "reserve"
	// UpstreamRewriteHost rewrites the client's host to the upstream.
	UpstreamRewriteHost = "rewrite"
)

// Checks the data of health check
type Checks struct {
	Active  *ActiveHealthCheck  `json:"active,omitempty"`
	Passive *PassiveHealthCheck `json:"passive,omitempty"`
}

// ActiveHealthCheck the data of active health check
type ActiveHealthCheck struct {
	Type  string                    `json:"type"`
	HTTP  *HTTPActiveHealthCheck    `json:"http"`
	HTTPS *HTTPSActiveHealthCheck   `json:"https"`
	TCP   *TCPActiveCheckPredicates `json:"tcp"`
}

// HTTPActiveHealthCheck is the configuration of HTTP active health check
type HTTPActiveHealthCheck struct {
	ProbeTimeout     int64                   `json:"probe_timeout,omitempty"`
	ConcurrentProbes int64                   `json:"concurrent_probes,omitempty"`
	HTTPProbePath    string                  `json:"http_probe_path,omitempty"`
	HTTPProbeHost    string                  `json:"http_probe_host,omitempty"`
	ProbeTargetPort  int64                   `json:"probe_target_port,omitempty"`
	HTTPProbeHeaders Header                  `json:"http_probe_headers,omitempty"`
	Healthy          HTTPHealthyPredicates   `json:"healthy,omitempty"`
	UnHealthy        HTTPUnhealthyPredicates `json:"unhealthy,omitempty"`
}

// Header is a custom HTTP headers struct for health check feature.
type Header map[string]string

func (header Header) ToStringArray() []string {
	var res []string
	if header == nil {
		return res
	}
	for k, v := range header {
		s := fmt.Sprintf("%v: %v", k, v)
		res = append(res, s)
	}
	return res
}

// HTTPSActiveHealthCheck the data of active health check for https
type HTTPSActiveHealthCheck struct {
	HTTPActiveHealthCheck

	VerifyTargetTlsCertificate bool `json:"verify_target_tls_certificate"`
}

// HTTPHealthyPredicates healthy predicates.
type HTTPHealthyPredicates struct {
	TargetsCheckInterval int64 `json:"targets_check_interval,omitempty"`
	HTTPStatusCodes      []int `json:"http_status_codes,omitempty"`
	Successes            int64 `json:"successes,omitempty"`
}

// HTTPUnhealthyPredicates unhealthy predicates.
type HTTPUnhealthyPredicates struct {
	TargetsCheckInterval int64 `json:"targets_check_interval,omitempty"`
	HTTPStatusCodes      []int `json:"http_status_codes,omitempty"`
	HTTPFailures         int64 `json:"http_failures,omitempty"`
	Timeouts             int64 `json:"timeouts,omitempty"`
}

// TCPActiveCheckPredicates predicates for the TCP probe active health check
type TCPActiveCheckPredicates struct {
	ProbeTimeout     int64                   `json:"probe_timeout,omitempty"`
	ConcurrentProbes int64                   `json:"concurrent_probes,omitempty"`
	ProbeTargetPort  int64                   `json:"probe_target_port,omitempty"`
	Healthy          *TCPHealthyPredicates   `json:"healthy,omitempty"`
	UnHealthy        *TCPUnhealthyPredicates `json:"unhealthy,omitempty"`
}

// TCPHealthyPredicates the healthy case data of tcp health check.
type TCPHealthyPredicates struct {
	TargetsCheckInterval int64 `json:"targets_check_interval,omitempty"`
	Successes            int64 `json:"successes,omitempty"`
}

// TCPUnhealthyPredicates the unhealthy case data of tcp health check.
type TCPUnhealthyPredicates struct {
	TargetsCheckInterval int64 `json:"targets_check_interval,omitempty"`
	TcpFailures          int64 `json:"tcp_failures,omitempty"`
	Timeouts             int64 `json:"timeouts,omitempty"`
}

// PassiveHealthCheck the data of passive health check
type PassiveHealthCheck struct {
	Type  string                     `json:"type"`
	HTTP  *HTTPPassiveHealthCheck    `json:"http"`
	HTTPS *HTTPPassiveHealthCheck    `json:"https"`
	TCP   *TCPPassiveCheckPredicates `json:"tcp"`
}

// HTTPPassiveHealthCheck is the configuration of HTTP passive health check
type HTTPPassiveHealthCheck struct {
	Healthy   HTTPHealthyPredicatesForPassive   `json:"healthy,omitempty"`
	UnHealthy HTTPUnhealthyPredicatesForPassive `json:"unhealthy,omitempty"`
}

// HTTPHealthyPredicatesForPassive healthy predicates for passive health check.
type HTTPHealthyPredicatesForPassive struct {
	HTTPStatusCodes []int `json:"http_status_codes,omitempty"`
}

// HTTPUnhealthyPredicatesForPassive unhealthy predicates for passive health check.
type HTTPUnhealthyPredicatesForPassive struct {
	HTTPStatusCodes []int `json:"http_status_codes,omitempty"`
	HTTPFailures    int64 `json:"http_failures,omitempty"`
	Timeouts        int64 `json:"timeouts,omitempty"`
}

// TCPPassiveCheckPredicates predicates for the TCP probe passive health check
type TCPPassiveCheckPredicates struct {
	UnHealthy *TCPUnhealthyPredicatesForPassive `json:"unhealthy,omitempty"`
}

// TCPUnhealthyPredicatesForPassive the unhealthy case data of passive tcp health check.
type TCPUnhealthyPredicatesForPassive struct {
	TcpFailures int64 `json:"tcp_failures,omitempty"`
	Timeouts    int64 `json:"timeouts,omitempty"`
}

// Configuration is the configuration of services
type Configuration struct {
	Name     string     `yaml:"name" json:"name"`
	Version  string     `yaml:"version" json:"version"`
	Services []*Service `yaml:"services" json:"services"`
	Routes   []*Route   `yaml:"routes" json:"routes"`
}

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
	Value        interface{}  `json:"value"`
}
