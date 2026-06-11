import {Injectable} from '@angular/core';
import {Observable, Subject} from 'rxjs';

@Injectable({
  providedIn: 'root',
})
export class WebsocketService {
  private socket?: WebSocket;
  private messagesSubject = new Subject<string>();

  messages$ = this.messagesSubject.asObservable();

  connect(url: string): void {
    if (this.socket?.readyState === WebSocket.OPEN) return;

    this.socket = new WebSocket(url);

    this.socket.onopen = () => {
      console.log('WebSocket verbunden');
    };

    this.socket.onmessage = (event) => {
      this.messagesSubject.next(event.data);
    };

    this.socket.onerror = (error) => {
      console.error('WebSocket Fehler:', error);
    };

    this.socket.onclose = () => {
      console.log('WebSocket geschlossen');
    };
  }

  send(message: string): void {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(message);
    } else {
      console.warn('WebSocket ist nicht verbunden');
    }
  }

  disconnect(): void {
    this.socket?.close();
  }
}
