FROM node:lts-bookworm-slim AS builder

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME/bin:$PATH"
ENV NX_DAEMON="false"
RUN corepack enable

WORKDIR /build

COPY . .

RUN pnpm install nx -g \
  && pnpm install \
  && NODE_ENV=production nx build cli

FROM gcr.io/distroless/nodejs24-debian13:nonroot

COPY --from=builder /build/dist/apps/cli/main.cjs .

ENTRYPOINT [ "/nodejs/bin/node", "main.cjs" ]
