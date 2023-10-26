package types

func PtrOf[T any](v T) *T {
	return &v
}

func SetRouteDefaultValues(route *Route) {
	if route.Priority == nil {
		route.Priority = PtrOf(0)
	}

	if route.Status == nil {
		route.Status = PtrOf(1)
	}

	if route.Upstream != nil {
		SetUpstreamDefaultValues(route.Upstream)
	}
}

func SetServiceDefaultValues(service *Service) {
	if service.Upstream != nil {
		SetUpstreamDefaultValues(service.Upstream)
	}
}

func SetStreamRouteDefaultValues(sr *StreamRoute) {
	if sr.Upstream != nil {
		SetUpstreamDefaultValues(sr.Upstream)

		if sr.Upstream.Scheme == "http" {
			sr.Upstream.Scheme = "tcp"
		}
	}
}

func SetUpstreamDefaultValues(upstream *Upstream) {
	if upstream.TLS != nil {
		if upstream.TLS.Verify == nil {
			upstream.TLS.Verify = PtrOf(false)
		}
	}

	if upstream.KeepalivePool != nil {
		if upstream.KeepalivePool.Size != nil {
			upstream.KeepalivePool.Size = PtrOf(1)
		}
		if upstream.KeepalivePool.IdleTimeout != nil {
			upstream.KeepalivePool.IdleTimeout = PtrOf(60)
		}
		if upstream.KeepalivePool.Requests != nil {
			upstream.KeepalivePool.Requests = PtrOf(1000)
		}

	}

	if len(upstream.Nodes) > 0 {
		for i := range upstream.Nodes {
			if upstream.Nodes[i].Priority == nil {
				upstream.Nodes[i].Priority = PtrOf(0)
			}
		}
	}

	if upstream.Checks != nil {
		SetHealthCheckerDefaultValues(upstream.Checks)
	}

	if upstream.Type == "" {
		upstream.Type = "roundrobin"
	}
	if upstream.HashOn == "" {
		upstream.HashOn = "vars"
	}
	if upstream.Scheme == "" {
		upstream.Scheme = "http"
	}
	if upstream.PassHost == "" {
		upstream.PassHost = "pass"
	}
}

func SetHealthCheckerDefaultValues(checker *UpstreamHealthCheck) {
	if checker.Active != nil {
		if checker.Active.Type == "" {
			checker.Active.Type = "http"
		}
		if checker.Active.Timeout == nil {
			checker.Active.Timeout = PtrOf(1)
		}
		if checker.Active.Concurrency == nil {
			checker.Active.Concurrency = PtrOf(10)
		}
		if checker.Active.HTTPPath == "" {
			checker.Active.HTTPPath = "/"
		}
		if checker.Active.HTTPSVerifyCert == nil {
			checker.Active.HTTPSVerifyCert = PtrOf(true)
		}

		// healthy
		if checker.Active.Healthy.Interval == nil {
			checker.Active.Healthy.Interval = PtrOf(1)
		}
		if checker.Active.Healthy.HTTPStatuses == nil {
			checker.Active.Healthy.HTTPStatuses = []int{200, 302}
		}
		if checker.Active.Healthy.Successes == nil {
			checker.Active.Healthy.Successes = PtrOf(2)
		}

		// unhealthy
		if checker.Active.Unhealthy.Interval == nil {
			checker.Active.Unhealthy.Interval = PtrOf(1)
		}
		if checker.Active.Unhealthy.HTTPStatuses == nil {
			checker.Active.Unhealthy.HTTPStatuses = []int{429, 404, 500, 501, 502, 503, 504, 505}
		}
		if checker.Active.Unhealthy.HTTPFailures == nil {
			checker.Active.Unhealthy.HTTPFailures = PtrOf(5)
		}
		if checker.Active.Unhealthy.TCPFailures == nil {
			checker.Active.Unhealthy.TCPFailures = PtrOf(2)
		}
		if checker.Active.Unhealthy.Timeouts == nil {
			checker.Active.Unhealthy.Timeouts = PtrOf(3)
		}
	}

	if checker.Passive != nil {
		if checker.Passive.Type == "" {
			checker.Passive.Type = "http"
		}

		// healthy
		if checker.Passive.Healthy.HTTPStatuses == nil {
			checker.Passive.Healthy.HTTPStatuses = []int{200, 201, 202, 203, 204, 205, 206, 207,
				208, 226, 300, 301, 302, 303, 304, 305,
				306, 307, 308}
		}
		if checker.Passive.Healthy.Successes == nil {
			checker.Passive.Healthy.Successes = PtrOf(5)
		}

		// unhealthy
		if checker.Passive.Unhealthy.HTTPStatuses == nil {
			checker.Passive.Unhealthy.HTTPStatuses = []int{429, 500, 503}
		}
		if checker.Passive.Unhealthy.HTTPFailures == nil {
			checker.Passive.Unhealthy.HTTPFailures = PtrOf(5)
		}
		if checker.Passive.Unhealthy.TCPFailures == nil {
			checker.Passive.Unhealthy.TCPFailures = PtrOf(2)
		}
		if checker.Passive.Unhealthy.Timeouts == nil {
			checker.Passive.Unhealthy.Timeouts = PtrOf(7)
		}
	}
}

func SetSSLDefaultValues(ssl *SSL) {
	if ssl.Type == "" {
		ssl.Type = "server"
	}
	if ssl.Client != nil {
		if ssl.Client.Depth == nil {
			ssl.Client.Depth = PtrOf(1)
		}
	}
	if ssl.Status == nil {
		ssl.Status = PtrOf(1)
	}
}
