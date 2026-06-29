// Cloudflare Pages Function — proxy /api/* to the Cloud Run backend.
//
// Why: the frontend (Cloudflare Pages) and backend (Cloud Run) live on
// different origins, but the session is an httpOnly cookie. Proxying the API
// through the Pages origin keeps the cookie FIRST-PARTY (no SameSite=None, no
// cross-site CORS), so auth + collaboration "just work". The browser only ever
// talks to the Pages domain; this function relays to Cloud Run server-side and
// passes Set-Cookie straight back.
//
// Config: set BACKEND_URL in the Pages project's env vars, e.g.
//   BACKEND_URL = https://canonforge-xxxxx-uc.a.run.app
export async function onRequest(context) {
  const { request, env, params } = context
  if (!env.BACKEND_URL) {
    return new Response('BACKEND_URL is not configured for this Pages project', { status: 500 })
  }
  const sub = Array.isArray(params.path) ? params.path.join('/') : params.path || ''
  const search = new URL(request.url).search
  const target = `${env.BACKEND_URL.replace(/\/$/, '')}/${sub}${search}`
  // Re-issue the request to the backend; method, headers (incl. Cookie), and
  // body are carried over. The response (incl. Set-Cookie) is returned as-is.
  return fetch(new Request(target, request))
}
