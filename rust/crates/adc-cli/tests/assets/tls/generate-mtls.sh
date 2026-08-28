#!/bin/bash
# Generates the self-signed CA/server/client cert chain these tests use.
# Uses -addext/-extfile to produce real X.509v3 certs (rustls/webpki reject v1).
set -euo pipefail
cd "$(dirname "$0")"

openssl genrsa -out ca.key 2048
openssl req -new -x509 -sha256 -days 36500 -key ca.key -out ca.cer -subj "/CN=ROOTCA" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign"

openssl genrsa -out server.key 2048
openssl req -new -sha256 -key server.key -out server.csr -subj "/CN=localhost"
openssl x509 -req -days 36500 -sha256 -CA ca.cer -CAkey ca.key -CAcreateserial -in server.csr -out server.cer \
  -extfile <(printf "basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth")

openssl genrsa -out client.key 2048
openssl req -new -sha256 -key client.key -out client.csr -subj "/CN=CLIENT"
openssl x509 -req -days 36500 -sha256 -CA ca.cer -CAkey ca.key -CAcreateserial -in client.csr -out client.cer \
  -extfile <(printf "basicConstraints=CA:FALSE\nextendedKeyUsage=clientAuth")

rm -f ca.srl
