package config

import "os"

var (
	TestUpstream = "httpbin.org"
)

func init() {
	if os.Getenv("GITHUB_ACTIONS") == "true" {
		TestUpstream = "httpbin"
	}
}
