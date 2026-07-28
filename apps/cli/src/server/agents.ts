import { HttpAgent, HttpOptions, HttpsAgent } from 'agentkeepalive';

// create connection pool
const keepAlive: HttpOptions = {
  keepAlive: true,
  maxSockets: 256, // per host
  maxFreeSockets: 16, // per host free
  freeSocketTimeout:
    parseInt(process.env.ADC_INGRESS_FREE_SOCKET_TIMEOUT ?? '') || 50000, // free socket keepalive for 50 seconds, and if the ADC_INGRESS_FREE_SOCKET_TIMEOUT environment variable is provided, it takes precedence.
};

export const httpAgent = new HttpAgent(keepAlive);

const httpsAgent = new HttpsAgent({
  rejectUnauthorized: true,
  ...keepAlive,
});
const httpsInsecureAgent = new HttpsAgent({
  rejectUnauthorized: false,
  ...keepAlive,
});

// one agent per CA bundle, so sockets stay pooled across requests. the key is
// request input, so the cache is capped and evicts least recently used first.
// evicted agents are dropped rather than destroyed: in-flight requests finish,
// and freeSocketTimeout reaps the sockets they leave behind.
export const MAX_CA_CERT_AGENTS = 32;
const httpsCACertAgents = new Map<string, HttpsAgent>();

export interface TLSOptions {
  tlsSkipVerify?: boolean;

  // PEM-encoded CA certificate (or bundle) to verify the backend against,
  // instead of the system trust store. Ignored when tlsSkipVerify is set.
  caCert?: string;
}

//TODO: support mTLS
export const resolveHttpsAgent = ({
  tlsSkipVerify,
  caCert,
}: TLSOptions): HttpsAgent => {
  if (tlsSkipVerify) return httpsInsecureAgent;

  const ca = caCert?.trim();
  if (!ca) return httpsAgent;

  const cached = httpsCACertAgents.get(ca);
  if (cached) {
    // re-insert to mark it most recently used
    httpsCACertAgents.delete(ca);
    httpsCACertAgents.set(ca, cached);
    return cached;
  }

  const agent = new HttpsAgent({
    rejectUnauthorized: true,
    ca,
    ...keepAlive,
  });
  httpsCACertAgents.set(ca, agent);

  if (httpsCACertAgents.size > MAX_CA_CERT_AGENTS) {
    const lruKey = httpsCACertAgents.keys().next().value;
    if (lruKey !== undefined) httpsCACertAgents.delete(lruKey);
  }

  return agent;
};
