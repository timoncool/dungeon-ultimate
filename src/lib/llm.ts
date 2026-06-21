// Resolve a user-entered backend URL to its /chat/completions endpoint.
// Accepts a bare host (http://127.0.0.1:8080), a versioned base (.../v1), or
// the full endpoint, so people can paste whatever their server prints.
export function customChatEndpoint(baseUrl: string): string {
  const url = baseUrl.trim().replace(/\/+$/, "");
  if (/\/chat\/completions$/.test(url)) return url;
  if (/\/v\d+$/.test(url)) return `${url}/chat/completions`;
  return `${url}/v1/chat/completions`;
}
