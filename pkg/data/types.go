package data

import (
	"fmt"
)

// StringArray is enhanced version of pq.StringArray that can be handled nil value automatically.
type StringArray []string

// Plugins contains a collect of polices like CORS.
type Plugins map[string]interface{}

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
