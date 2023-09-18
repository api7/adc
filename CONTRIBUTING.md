# Contributing to ADC

All kinds of contributions are welcome! Whether it is:

- Reporting a bug
- Proposing a new feature
- Improving documentation
- Submitting a fix

## Reporting Bugs and Suggesting New Features

We use [GitHub issues](https://github.com/api7/adc/issues?q=is%3Aissue+is%3Aopen+sort%3Aupdated-desc) to track bugs and feature requests. Please make sure your requests are as detailed as possible to ensure minimum turnaround time.

## Proposing Changes

Pull requests are the best way to propose changes to ADC. Please follow these steps while submitting a pull request:

1. Fork the repo and create your branch from main.
2. If you've added code that should be tested, add tests.
3. If you've changed behavior, update the [documentation](./README.md).
4. Open that pull request!
5. Ensure the test suites pass in CI.

See [GitHub Flow](https://guides.github.com/introduction/flow/index.html).

## Building ADC Locally

To make changes to the code, you need to build ADC locally.

First, you need to install [Go](https://go.dev/) in your development environment. ADC uses Go v1.20.

Then fork and clone the repository, navigate to it in your machine and run:

```shell
make build
```

## Connecting to APISIX

ADC works with [Apache APISIX](https://docs.api7.ai/apisix). To deploy APISIX in Docker, run:

```shell
curl -sL https://run.api7.ai/apisix/quickstart | sh
```

Once APISIX is deployed and ready, ADC can be configured to connect to it by running:

```shell
./bin/adc configure
```

You can add the APISIX endpoint address, which is by default `http://127.0.0.1:9180`, when prompted.

## Testing ADC

Now to test a particular command, for example `ping`, you can run:

```shell
./bin/adc ping
```

You can also run the Go code directly without building the binary:

```shell
go run main.go ping
```

To run unit tests and e2e test suites:

```shell
make unit-test
make test
```

ADC uses the [Cobra](https://github.com/spf13/cobra) library.
