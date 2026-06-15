/**
 * Baut eine absolute WebSocket-URL aus einem relativen Pfad, basierend auf dem
 * Host, von dem die App ausgeliefert wird. Dadurch funktioniert derselbe Build
 * hinter dem Dev-Proxy (ng serve), dem Ingress im Cluster und ngrok (wss).
 */
export function wsUrl(path: string): string {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${location.host}${path}`;
}
