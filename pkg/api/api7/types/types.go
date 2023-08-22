package types

import "github.com/api7/adc/pkg/data"

// Service is the abstraction of a backend service on API gateway.
type Service struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	// Labels are used for resource classification and indexing
	Labels data.StringArray `json:"labels,omitempty"`
	// Protocols indicates the protocols that the service supports
	Protocols []string `json:"protocols,omitempty"`
	// The listening path prefix for this service.
	PathPrefix string `json:"path_prefix,omitempty"`
	// HTTP hosts for this service.
	Hosts []string `json:"hosts"`
	// Plugin settings on Service level
	Plugins data.Plugins `json:"plugins,omitempty"`
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
	HTTPProbeHeaders data.Header             `json:"http_probe_headers,omitempty"`
	Healthy          HTTPHealthyPredicates   `json:"healthy,omitempty"`
	UnHealthy        HTTPUnhealthyPredicates `json:"unhealthy,omitempty"`
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

// ServiceConfig is the configuration of services
type Configuration struct {
	Services []Service `yaml:"services" json:"services"`
}
