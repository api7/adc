#!/bin/bash
set -e

# Uses `-addext` (OpenSSL 1.1.1+) to set v3 extensions inline instead of
# `-extensions v3_ca`/`v3_req`, which only work if the system's default
# openssl.cnf happens to define matching [v3_ca]/[v3_req] sections — it
# often doesn't, silently producing a v1 certificate with no extensions at
# all. rustls's webpki validator (used by the Rust e2e suite's HTTP client)
# rejects v1 trust anchors outright, unlike Node's OpenSSL-backed TLS stack
# which accepts them; -addext keeps this reproducible regardless of the
# host's config.

# For ROOT CA
openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 36500 \
  -keyout ca.key -out ca.cer -subj "/CN=ROOTCA" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,keyCertSign,cRLSign"

# For server certificate
openssl genrsa -out server.key 2048
openssl req -new -sha256 -key server.key -out server.csr -subj "/CN=localhost"
openssl x509 -req -sha256 -days 36500 -in server.csr -CA ca.cer -CAkey ca.key -CAserial ca.srl -CAcreateserial \
  -out server.cer \
  -extfile <(printf "basicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost")

# For client certificate
openssl genrsa -out client.key 2048
openssl req -new -sha256 -key client.key -out client.csr -subj "/CN=CLIENT"
openssl x509 -req -sha256 -days 36500 -in client.csr -CA ca.cer -CAkey ca.key -CAserial ca.srl -CAcreateserial \
  -out client.cer \
  -extfile <(printf "basicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth")

chmod -R 777 .
