import { MemoryStore, RedisStore, type Store } from './store';
export type Env = Record<string, string | undefined>;
export const NEARAI_DEFAULT_BASE_URL = 'https://cloud-api.near.ai/v1';
export const NEARAI_DEFAULT_MODEL = 'Qwen/Qwen3.6-35B-A3B-FP8';
export const TESTNET_DEFAULT_RPC = 'https://test.rpc.fastnear.com';
export type Config = ReturnType<typeof loadConfig>;
const int = (v: string | undefined, d: number) => { const n = Number(v); return Number.isInteger(n) && n > 0 ? n : d; };
export function loadConfig(env: Env) {
  const secret = (name: string) => { const v = env[name]; return v && v.length >= 32 ? v : ''; };
  return {
    nearAiKey: env.NEARAI_API_KEY ?? '',
    nearAiBaseUrl: (env.NEARAI_BASE_URL || NEARAI_DEFAULT_BASE_URL).replace(/\/$/, ''),
    nearAiModel: env.NEARAI_MODEL || NEARAI_DEFAULT_MODEL,
    chatSecret: secret('CHAT_HMAC_SECRET'),
    sessionSecret: secret('SESSION_SECRET'),
    allowedOrigins: (env.ALLOWED_ORIGINS ?? '').split(',').map(v => v.trim()).filter(Boolean),
    adminWallet: env.ADMIN_WALLET ?? '',
    rpcUrl: env.NEAR_RPC_URL || TESTNET_DEFAULT_RPC,
    cronSecret: secret('CRON_SECRET'),
    chatSessionCap: int(env.CHAT_SESSION_CAP, 12),
    chatDayCap: int(env.CHAT_DAY_CAP, 40),
    chatGlobalDayCap: int(env.CHAT_GLOBAL_DAY_CAP, 800),
    storageMode: env.STORAGE_MODE,
    redisUrl: env.KV_REST_API_URL || env.UPSTASH_REDIS_REST_URL || '',
    redisToken: env.KV_REST_API_TOKEN || env.UPSTASH_REDIS_REST_TOKEN || '',
  };
}
let memory: MemoryStore | undefined;
/** Production needs Upstash. Memory is only for explicit local runs and tests. */
export function storeFor(config: Config): Store | null {
  if (config.redisUrl && config.redisToken) return new RedisStore(config.redisUrl, config.redisToken);
  if (config.storageMode === 'memory') return memory ??= new MemoryStore();
  return null;
}
