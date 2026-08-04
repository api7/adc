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

// key: sha256 fingerprint of the TLS material -> value: a pooled HttpsAgent
const httpsAgentPool = new LRUCache<string, HttpsAgent>({
  max: maxPoolSize,
  // an evicted agent may still hold open keep-alive sockets; nothing else
  // references it once it leaves the pool, so it must be destroyed here or
  // its sockets/fds would leak
  dispose: (agent) => agent.destroy(),
});

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
 */
export const getHttpsAgent = (tls: TlsMaterial = {}): HttpsAgent => {
  const key = fingerprintTlsMaterial(tls);
  const cached = httpsAgentPool.get(key); // also refreshes LRU recency
  if (cached) return cached;

  const agent = new HttpsAgent({
    ...keepAlive,
    rejectUnauthorized: !tls.tlsSkipVerify,
    ...(tls.caCert ? { ca: tls.caCert } : {}),
    ...(tls.tlsClientCert ? { cert: tls.tlsClientCert } : {}),
    ...(tls.tlsClientKey ? { key: tls.tlsClientKey } : {}),
  });
  httpsAgentPool.set(key, agent);
  return agent;
};
