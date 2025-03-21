FROM node:lts-bookworm-slim AS builder

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable

WORKDIR /build

COPY . .

RUN pnpm install nx -g \
  && pnpm install \
  && NODE_ENV=production nx build cli

FROM node:lts-bookworm-slim

COPY --from=builder /build/dist/apps/cli/main.js .

ENTRYPOINT [ "node", "main.js" ]
