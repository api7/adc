/*
Copyright © 2023 API7.ai
*/
package config

type ClientConfig struct {
	Server string
	Token  string

	CAPath         string
	Certificate    string
	CertificateKey string
	Insecure       bool
}
