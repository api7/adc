import { HttpAgent, HttpOptions, HttpsAgent } from 'agentkeepalive';
import { LRUCache } from 'lru-cache';
import { createHash } from 'node:crypto';

const keepAlive: HttpOptions = {
  keepAlive: true,
  maxSockets: 256, // per host
  maxFreeSockets: 16, // per host free
  freeSocketTimeout:
    parseInt(process.env.ADC_INGRESS_FREE_SOCKET_TIMEOUT ?? '') || 50000, // free socket keepalive for 50 seconds, and if the ADC_INGRESS_FREE_SOCKET_TIMEOUT environment variable is provided, it takes precedence.
};

// plain http:// backends have no TLS material to distinguish, so a single
// shared agent is enough
export const httpAgent = new HttpAgent(keepAlive);

export interface TlsMaterial {
  tlsSkipVerify?: boolean;
  caCert?: string;
  tlsClientCert?: string;
  tlsClientKey?: string;
}

const parseEnvInt = (value: string | undefined, defaultVal: number): number => {
  const n = Number(value ?? defaultVal);
  return Number.isFinite(n) && n >= 1 ? Math.floor(n) : defaultVal;
};
const maxPoolSize = parseEnvInt(process.env.ADC_INGRESS_TLS_AGENT_POOL_MAX, 16);

interface PooledAgent {
  agent: HttpsAgent;
  activeRequests: number;
  evicted: boolean;
}

// only safe to close an evicted agent's sockets once nothing is still using
// it for an in-flight request
const destroyIfIdle = (entry: PooledAgent) => {
  if (entry.evicted && entry.activeRequests === 0) entry.agent.destroy();
};

// key: sha256 fingerprint of the TLS material -> value: a pooled agent entry
const httpsAgentPool = new LRUCache<string, PooledAgent>({
  max: maxPoolSize,
  dispose: (entry) => {
    entry.evicted = true;
    destroyIfIdle(entry);
  },
});

// reverse lookup so releaseHttpsAgent can find an agent's bookkeeping entry
// even after it has been evicted from httpsAgentPool
const entriesByAgent = new WeakMap<HttpsAgent, PooledAgent>();

// Fingerprint the TLS material into a fixed-size cache key instead of using
// the raw PEM strings as the Map key. `\0` separators avoid ambiguous
// concatenation collisions between fields.
export const fingerprintTlsMaterial = (tls: TlsMaterial = {}): string =>
  createHash('sha256')
    .update(tls.tlsSkipVerify ? '1' : '0')
    .update('\0')
    .update(tls.caCert ?? '')
    .update('\0')
    .update(tls.tlsClientCert ?? '')
    .update('\0')
    .update(tls.tlsClientKey ?? '')
    .digest('hex');

/**
 * Returns a pooled HttpsAgent for the given TLS material. Requests with
 * identical material share (and thus keep-alive-reuse) the same agent and
 * connection pool; different material gets an isolated agent so certs/keys
 * are never cross-contaminated between backends.
 *
 * Every call must be paired with a `releaseHttpsAgent` call once the request
 * using the returned agent has finished, so an agent evicted from the pool
 * while still in flight isn't destroyed out from under that request.
 */
export const getHttpsAgent = (tls: TlsMaterial = {}): HttpsAgent => {
  const key = fingerprintTlsMaterial(tls);
  let entry = httpsAgentPool.get(key); // also refreshes LRU recency
  if (!entry) {
    const agent = new HttpsAgent({
      ...keepAlive,
      rejectUnauthorized: !tls.tlsSkipVerify,
      ...(tls.caCert ? { ca: tls.caCert } : {}),
      ...(tls.tlsClientCert ? { cert: tls.tlsClientCert } : {}),
      ...(tls.tlsClientKey ? { key: tls.tlsClientKey } : {}),
    });
    entry = { agent, activeRequests: 0, evicted: false };
    httpsAgentPool.set(key, entry);
    entriesByAgent.set(agent, entry);
  }
  entry.activeRequests++;
  return entry.agent;
};

/** Releases an agent obtained from `getHttpsAgent` once its request is done. */
export const releaseHttpsAgent = (agent: HttpsAgent): void => {
  const entry = entriesByAgent.get(agent);
  if (!entry) return;
  entry.activeRequests = Math.max(0, entry.activeRequests - 1);
  destroyIfIdle(entry);
};
