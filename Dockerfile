# syntax=docker/dockerfile:1

ARG GO_VERSION=1.20
ARG VERSION=dev
FROM golang:${GO_VERSION} AS build
WORKDIR /src

RUN --mount=type=cache,target=/go/pkg/mod/ \
    --mount=type=bind,source=go.sum,target=go.sum \
    --mount=type=bind,source=go.mod,target=go.mod \
    go mod download -x

RUN --mount=type=cache,target=/go/pkg/mod/ \
    --mount=type=bind,target=. \
    CGO_ENABLED=0 go build -o /bin/adc -ldflags "-X github.com/api7/adc/cmd.VERSION=$VERSION -X github.com/api7/adc/cmd.GitRevision=$VERSION" .

FROM alpine:3.18 AS final

# Copy the executable from the "build" stage.
COPY --from=build --chown=appuser:appuser /bin/adc /bin/

CMD /bin/adc
