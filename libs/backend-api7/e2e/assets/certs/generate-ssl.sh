#!/bin/bash

openssl genrsa -out test-ssl1.key 2048
openssl req -new -sha256 -key test-ssl1.key -out test-ssl1.csr -subj "/CN=test1"
openssl x509 -req -days 36500 -sha256 -extensions v3_ca -signkey test-ssl1.key -in test-ssl1.csr -out test-ssl1.cer

openssl genrsa -out test-ssl2.key 2048
openssl req -new -sha256 -key test-ssl2.key -out test-ssl2.csr -subj "/CN=test2"
openssl x509 -req -days 36500 -sha256 -extensions v3_ca -signkey test-ssl2.key -in test-ssl2.csr -out test-ssl2.cer
