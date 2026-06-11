import { Injectable } from '@angular/core';
import { Subject } from 'rxjs';

@Injectable({
  providedIn: 'root',
})
export class WebsocketService {
  private socket?: WebSocket;
  private messagesSubject = new Subject<string>();
  private connectedSubject = new Subject<void>();

  messages$ = this.messagesSubject.asObservable();
  connected$ = this.connectedSubject.asObservable();

  connect(url: string): void {
    if (this.socket?.readyState === WebSocket.OPEN) return;

    this.socket = new WebSocket(url);

    this.socket.onopen = () => {
      console.log('[WS] Verbunden mit:', url);
      this.connectedSubject.next();
    };

    this.socket.onmessage = (event) => {
      console.log('[WS] Nachricht empfangen:', event.data);
      this.messagesSubject.next(event.data);
    };

    this.socket.onerror = (error) => {
      console.error('[WS] Fehler:', error);
    };

    this.socket.onclose = (event) => {
      console.log('[WS] Geschlossen — Code:', event.code, 'Grund:', event.reason);
    };
  }

  public send(message: string): void {
    if (this.socket?.readyState === WebSocket.OPEN) {
      console.log('[WS] Gesendet:', message);
      this.socket.send(message);
    } else {
      console.warn('[WS] Nicht verbunden — Nachricht verworfen:', message);
    }
  }

  disconnect(): void {
    this.socket?.close();
  }
}
