package config

import (
	"os"
	"strings"
)

var (
	TestUpstream = "httpbin.org"
)

func ReplaceUpstream(conf string) string {
	return strings.ReplaceAll(conf, "HTTPBIN_PLACEHOLDER", TestUpstream)
}

func init() {
	if os.Getenv("GITHUB_ACTIONS") == "true" {
		TestUpstream = "httpbin"
	}
}
