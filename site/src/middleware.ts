import { defineMiddleware } from 'astro:middleware';
import { withWalletCsp } from './server/csp';
/** Wallet sign in pages get a fresh nonce CSP per response. Every other route keeps the strict vercel.json policy. */
export const onRequest = defineMiddleware((context, next) => withWalletCsp(context.url.pathname, context.isPrerendered, context.locals, next));
