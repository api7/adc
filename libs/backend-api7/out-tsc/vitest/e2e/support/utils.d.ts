import * as ADCSDK from '@api7/adc-sdk';
import { Agent as httpsAgent } from 'node:https';
import * as semver from 'semver';
import { BackendAPI7 } from '../../src';
export declare const syncEvents: (backend: BackendAPI7, events?: Array<ADCSDK.Event>, opts?: ADCSDK.BackendSyncOptions) => Promise<ADCSDK.BackendSyncResult[]>;
export declare const dumpConfiguration: (backend: BackendAPI7) => Promise<{
    services?: {
        name: string;
        id?: string | undefined;
        description?: string | undefined;
        labels?: Record<string, string | string[]> | undefined;
        upstream?: {
            name?: string | undefined;
            description?: string | undefined;
            labels?: Record<string, string | string[]> | undefined;
            type?: "roundrobin" | "chash" | "least_conn" | "ewma" | undefined;
            hash_on?: string | undefined;
            key?: string | undefined;
            checks?: {
                active: {
                    type?: "http" | "https" | "tcp" | undefined;
                    timeout?: number | undefined;
                    concurrency?: number | undefined;
                    host?: string | undefined;
                    port?: number | undefined;
                    http_path?: string | undefined;
                    https_verify_cert?: boolean | undefined;
                    http_request_headers?: string[] | undefined;
                    healthy?: {
                        interval: number;
                        http_statuses?: number[] | undefined;
                        successes?: number | undefined;
                    } | undefined;
                    unhealthy?: {
                        interval: number;
                        http_statuses?: number[] | undefined;
                        http_failures?: number | undefined;
                        tcp_failures?: number | undefined;
                        timeouts?: number | undefined;
                    } | undefined;
                };
                passive?: {
                    type?: "http" | "https" | "tcp" | undefined;
                    healthy?: {
                        http_statuses?: number[] | undefined;
                        successes?: number | undefined;
                    } | undefined;
                    unhealthy?: {
                        http_statuses?: number[] | undefined;
                        http_failures?: number | undefined;
                        tcp_failures?: number | undefined;
                        timeouts?: number | undefined;
                    } | undefined;
                } | undefined;
            } | undefined;
            nodes?: {
                host: string;
                port: number;
                weight: number;
                priority?: number | undefined;
                metadata?: Record<string, any> | undefined;
            }[] | undefined;
            scheme?: "http" | "https" | "tcp" | "grpc" | "grpcs" | "tls" | "udp" | "kafka" | undefined;
            retries?: number | undefined;
            retry_timeout?: number | undefined;
            timeout?: {
                connect: number;
                send: number;
                read: number;
            } | undefined;
            tls?: {
                client_cert?: string | undefined;
                client_key?: string | undefined;
                client_cert_id?: string | undefined;
                verify?: boolean | undefined;
            } | undefined;
            keepalive_pool?: {
                size: number;
                idle_timeout: number;
                requests: number;
            } | undefined;
            pass_host?: "pass" | "node" | "rewrite" | undefined;
            upstream_host?: string | undefined;
            service_name?: string | undefined;
            discovery_type?: string | undefined;
            discovery_args?: Record<string, any> | undefined;
        } | undefined;
        upstreams?: {
            name?: string | undefined;
            description?: string | undefined;
            labels?: Record<string, string | string[]> | undefined;
            type?: "roundrobin" | "chash" | "least_conn" | "ewma" | undefined;
            hash_on?: string | undefined;
            key?: string | undefined;
            checks?: {
                active: {
                    type?: "http" | "https" | "tcp" | undefined;
                    timeout?: number | undefined;
                    concurrency?: number | undefined;
                    host?: string | undefined;
                    port?: number | undefined;
                    http_path?: string | undefined;
                    https_verify_cert?: boolean | undefined;
                    http_request_headers?: string[] | undefined;
                    healthy?: {
                        interval: number;
                        http_statuses?: number[] | undefined;
                        successes?: number | undefined;
                    } | undefined;
                    unhealthy?: {
                        interval: number;
                        http_statuses?: number[] | undefined;
                        http_failures?: number | undefined;
                        tcp_failures?: number | undefined;
                        timeouts?: number | undefined;
                    } | undefined;
                };
                passive?: {
                    type?: "http" | "https" | "tcp" | undefined;
                    healthy?: {
                        http_statuses?: number[] | undefined;
                        successes?: number | undefined;
                    } | undefined;
                    unhealthy?: {
                        http_statuses?: number[] | undefined;
                        http_failures?: number | undefined;
                        tcp_failures?: number | undefined;
                        timeouts?: number | undefined;
                    } | undefined;
                } | undefined;
            } | undefined;
            nodes?: {
                host: string;
                port: number;
                weight: number;
                priority?: number | undefined;
                metadata?: Record<string, any> | undefined;
            }[] | undefined;
            scheme?: "http" | "https" | "tcp" | "grpc" | "grpcs" | "tls" | "udp" | "kafka" | undefined;
            retries?: number | undefined;
            retry_timeout?: number | undefined;
            timeout?: {
                connect: number;
                send: number;
                read: number;
            } | undefined;
            tls?: {
                client_cert?: string | undefined;
                client_key?: string | undefined;
                client_cert_id?: string | undefined;
                verify?: boolean | undefined;
            } | undefined;
            keepalive_pool?: {
                size: number;
                idle_timeout: number;
                requests: number;
            } | undefined;
            pass_host?: "pass" | "node" | "rewrite" | undefined;
            upstream_host?: string | undefined;
            service_name?: string | undefined;
            discovery_type?: string | undefined;
            discovery_args?: Record<string, any> | undefined;
        }[] | undefined;
        plugins?: Record<string, {
            [x: string]: unknown;
        }> | undefined;
        path_prefix?: string | undefined;
        strip_path_prefix?: boolean | undefined;
        hosts?: string[] | undefined;
        routes?: {
            name: string;
            uris: string[];
            id?: string | undefined;
            description?: string | undefined;
            labels?: Record<string, string | string[]> | undefined;
            hosts?: string[] | undefined;
            priority?: number | undefined;
            timeout?: {
                connect: number;
                send: number;
                read: number;
            } | undefined;
            vars?: any[] | undefined;
            methods?: ("GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "TRACE" | "CONNECT" | "PURGE")[] | undefined;
            enable_websocket?: boolean | undefined;
            remote_addrs?: string[] | undefined;
            plugins?: Record<string, {
                [x: string]: unknown;
            }> | undefined;
            filter_func?: string | undefined;
        }[] | undefined;
        stream_routes?: {
            name: string;
            id?: string | undefined;
            description?: string | undefined;
            labels?: Record<string, string | string[]> | undefined;
            plugins?: Record<string, {
                [x: string]: unknown;
            }> | undefined;
            remote_addr?: string | undefined;
            server_addr?: string | undefined;
            server_port?: number | undefined;
            sni?: string | undefined;
        }[] | undefined;
    }[] | undefined;
    ssls?: {
        snis: string[];
        certificates: {
            certificate: string;
            key: string;
        }[];
        id?: string | undefined;
        labels?: Record<string, string | string[]> | undefined;
        type?: "server" | "client" | undefined;
        client?: {
            ca: string;
            depth?: number | undefined;
            skip_mtls_uri_regex?: string[] | undefined;
        } | undefined;
        ssl_protocols?: ("TLSv1.1" | "TLSv1.2" | "TLSv1.3")[] | undefined;
    }[] | undefined;
    consumers?: {
        username: string;
        description?: string | undefined;
        labels?: Record<string, string | string[]> | undefined;
        plugins?: Record<string, {
            [x: string]: unknown;
        }> | undefined;
        credentials?: {
            name: string;
            type: string;
            config: {
                [x: string]: unknown;
            };
            id?: string | undefined;
            description?: string | undefined;
            labels?: Record<string, string | string[]> | undefined;
        }[] | undefined;
    }[] | undefined;
    consumer_groups?: {
        name: string;
        plugins: Record<string, {
            [x: string]: unknown;
        }>;
        id?: string | undefined;
        description?: string | undefined;
        labels?: Record<string, string | string[]> | undefined;
        consumers?: {
            username: string;
            description?: string | undefined;
            labels?: Record<string, string | string[]> | undefined;
            plugins?: Record<string, {
                [x: string]: unknown;
            }> | undefined;
            credentials?: {
                name: string;
                type: string;
                config: {
                    [x: string]: unknown;
                };
                id?: string | undefined;
                description?: string | undefined;
                labels?: Record<string, string | string[]> | undefined;
            }[] | undefined;
        }[] | undefined;
    }[] | undefined;
    global_rules?: Record<string, {
        [x: string]: unknown;
    }> | undefined;
    plugin_metadata?: Record<string, {
        [x: string]: unknown;
    }> | undefined;
}>;
export declare const getDefaultValue: (backend: BackendAPI7) => Promise<ADCSDK.DefaultValue>;
export declare const createEvent: (resourceType: ADCSDK.ResourceType, resourceName: string, resource: object, parentName?: string) => ADCSDK.Event;
export declare const updateEvent: (resourceType: ADCSDK.ResourceType, resourceName: string, resource: object, parentName?: string) => ADCSDK.Event;
export declare const deleteEvent: (resourceType: ADCSDK.ResourceType, resourceName: string, parentName?: string) => ADCSDK.Event;
export declare const overrideEventResourceId: (event: ADCSDK.Event, resourceId: string, parentId?: string) => ADCSDK.Event<any>;
export declare const sortResult: <T>(result: Array<T>, field: string) => T[];
export declare const generateHTTPSAgent: () => httpsAgent;
type cond = boolean | (() => boolean);
export declare const conditionalDescribe: (cond: cond) => typeof describe | typeof describe.skip;
export declare const conditionalIt: (cond: cond) => typeof it | typeof it.skip;
export declare const semverCondition: (op: (v1: string | semver.SemVer, v2: string | semver.SemVer) => boolean, base: string, target?: string | semver.SemVer) => boolean;
export {};
//# sourceMappingURL=utils.d.ts.map