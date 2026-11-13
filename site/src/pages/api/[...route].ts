import type { APIRoute } from 'astro';
import { handle } from '../../server/api';
import { loadConfig } from '../../server/config';
import { nearAiModel } from '../../server/chat';
export const prerender = false;
export const ALL: APIRoute = ({ request, params, clientAddress }) => {
  const config = loadConfig(process.env);
  return handle(request, params.route ?? '', { config, model: nearAiModel(config), address: clientAddress });
};
