// Cloudflare Pages Function — proxy /api/* to the Cloud Run backend.
//
// Why: the frontend (Cloudflare Pages) and backend (Cloud Run) live on
// different origins, but the session is an httpOnly cookie. Proxying the API
// through the Pages origin keeps the cookie FIRST-PARTY (no SameSite=None, no
// cross-site CORS), so auth + collaboration "just work". The browser only ever
// talks to the Pages domain; this function relays to Cloud Run server-side and
// passes Set-Cookie straight back.
//
// Backend: the deployed Cloud Run URL. Override per-environment by setting
// BACKEND_URL in the Pages project's env vars; otherwise this default is used.
const DEFAULT_BACKEND = 'https://canonforge-7piuddab4q-as.a.run.app'

export async function onRequest(context) {
  const { request, env, params } = context
  const backend = (env.BACKEND_URL || DEFAULT_BACKEND).replace(/\/$/, '')
  const sub = Array.isArray(params.path) ? params.path.join('/') : params.path || ''
  const search = new URL(request.url).search
  const target = `${backend}/${sub}${search}`
  // Re-issue the request to the backend; method, headers (incl. Cookie), and
  // body are carried over. The response (incl. Set-Cookie) is returned as-is.
  return fetch(new Request(target, request))
}
