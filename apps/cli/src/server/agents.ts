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

// one agent per CA bundle, so sockets stay pooled across requests.
// keyed by the bundle itself; the key space is bounded by the number of
// distinct backends the server talks to.
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
  if (!caCert) return httpsAgent;

  const cached = httpsCACertAgents.get(caCert);
  if (cached) return cached;

  const agent = new HttpsAgent({
    rejectUnauthorized: true,
    ca: caCert,
    ...keepAlive,
  });
  httpsCACertAgents.set(caCert, agent);
  return agent;
};
