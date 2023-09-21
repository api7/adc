#!/bin/bash

openssl genrsa -out ca.key 2048
openssl req -new -sha256 -key ca.key -out ca.csr -subj "/CN=ROOTCA"
openssl x509 -req -days 36500 -sha256 -extensions v3_ca -signkey ca.key -in ca.csr -out ca.cert

# Server certs
openssl genrsa -out server.key 2048
openssl req -new -sha256 -key server.key -out server.csr -subj "/CN=127.0.0.1"
openssl x509 -req -days 36500 -sha256 -extensions v3_req  -CA  ca.cert -CAkey ca.key  -CAserial ca.srl  -CAcreateserial -in server.csr -out server.cert

# Client certs
openssl genrsa -out client.key 2048
openssl req -new -sha256 -key client.key  -out client.csr -subj "/CN=CLIENT"
openssl x509 -req -days 36500 -sha256 -extensions v3_req  -CA  ca.cert -CAkey ca.key  -CAserial ca.srl  -CAcreateserial -in client.csr -out client.cert

# Concat CA
cat ca.cert > ca.ca
cat ca.key >> ca.ca

# Remove unused files
rm -f ca.csr ca.srl client.csr server.csr
