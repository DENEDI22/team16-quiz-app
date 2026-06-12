import { computed, inject, Injectable, signal } from '@angular/core';
import { Router } from '@angular/router';
import { Subject, Subscription } from 'rxjs';
import {
  DuelGameOverMsg,
  DuelQuestionMsg,
  DuelQuestionResultMsg,
  DuelServerMsg,
  PlayerInfo,
} from '../../models/lobby';
import { AuthService } from '../auth/auth';
import { WebsocketService } from '../ws/WebsocketService';
import { MULTIPLAYER_WS_URL } from './lobby-service';

function decodeHtml(html: string): string {
  const txt = document.createElement('textarea');
  txt.innerHTML = html;
  return txt.value;
}

export type DuelStatus = 'connecting' | 'waiting' | 'playing' | 'over';

const QUESTION_SECONDS = 5;
const MAX_RECONNECT_ATTEMPTS = 8;

@Injectable({
  providedIn: 'root',
})
export class DuelService {
  private websocket = inject(WebsocketService);
  private authService = inject(AuthService);
  private router = inject(Router);
  private subscriptions = new Subscription();
  private countdownTimer: ReturnType<typeof setInterval> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  /** Wollen wir gerade verbunden sein? false nach cleanup/Spielende/Fehler. */
  private wantConnection = false;
  /** Sofort gesetzt (ohne Anzeige-Delay), damit der Close nach game_over
   *  keinen Reconnect auslöst. */
  private gameOverReceived = false;
  private wsUrl = '';

  status = signal<DuelStatus>('connecting');
  host = signal<PlayerInfo | null>(null);
  guest = signal<PlayerInfo | null>(null);
  totalQuestions = signal(0);
  hostScore = signal(0);
  guestScore = signal(0);
  currentQuestion = signal<DuelQuestionMsg | null>(null);
  lastResult = signal<DuelQuestionResultMsg | null>(null);
  gameOver = signal<DuelGameOverMsg | null>(null);
  opponentOffline = signal(false);
  errorMessage = signal('');
  /** Verbleibende Antwortzeit der aktuellen Frage in Sekunden (Anzeige). */
  secondsLeft = signal(QUESTION_SECONDS);
  /** true, während die Verbindung nach einem Abbruch neu aufgebaut wird. */
  reconnecting = signal(false);

  myUserId = signal('');
  isHost = computed(() => this.myUserId() !== '' && this.myUserId() === this.host()?.id);
  myScore = computed(() => (this.isHost() ? this.hostScore() : this.guestScore()));
  opponentScore = computed(() => (this.isHost() ? this.guestScore() : this.hostScore()));
  opponent = computed(() => (this.isHost() ? this.guest() : this.host()));

  /** Ergebnis des eigenen Spielers aus dem letzten Fragenergebnis. */
  myResult = computed(() => {
    const result = this.lastResult();
    if (!result) return null;
    return this.isHost() ? result.hostResult : result.guestResult;
  });
  opponentResult = computed(() => {
    const result = this.lastResult();
    if (!result) return null;
    return this.isHost() ? result.guestResult : result.hostResult;
  });

  /** Feuert synchron pro Fragenergebnis — wie answerResult$ im GameService. */
  readonly questionResult$ = new Subject<DuelQuestionResultMsg>();

  connect(lobbyId: string): void {
    this.subscriptions.unsubscribe();
    this.subscriptions = new Subscription();
    this.stopCountdown();
    this.stopReconnectTimer();
    this.wantConnection = true;
    this.gameOverReceived = false;
    this.reconnectAttempts = 0;
    this.reconnecting.set(false);
    this.wsUrl = `${MULTIPLAYER_WS_URL}/duels/${lobbyId}/ws`;

    this.status.set('connecting');
    this.host.set(null);
    this.guest.set(null);
    this.totalQuestions.set(0);
    this.hostScore.set(0);
    this.guestScore.set(0);
    this.currentQuestion.set(null);
    this.lastResult.set(null);
    this.gameOver.set(null);
    this.opponentOffline.set(false);
    this.errorMessage.set('');
    this.myUserId.set(this.readUserIdFromToken());

    this.subscriptions.add(
      this.websocket.connected$.subscribe(() => {
        this.reconnectAttempts = 0;
        this.reconnecting.set(false);
        const token = localStorage.getItem('token') ?? '';
        this.websocket.send(JSON.stringify({ type: 'hello', token }));
      }),
    );
    this.subscriptions.add(this.websocket.messages$.subscribe((raw) => this.handleMessage(raw)));
    this.subscriptions.add(this.websocket.closed$.subscribe(() => this.handleClosed()));

    this.websocket.connect(this.wsUrl);
  }

  /**
   * Auto-Reconnect mit Backoff. Der Server kann eine laufende Partie nahtlos
   * fortsetzen (Resumed-Nachricht) — sogar nach einem Service-Neustart, dank
   * Redis-Checkpoint. Kein Reconnect nach Spielende, Server-Fehler oder
   * absichtlichem Verlassen.
   */
  private handleClosed(): void {
    if (!this.wantConnection || this.gameOverReceived || this.errorMessage() !== '') {
      return;
    }
    if (this.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
      this.reconnecting.set(false);
      this.errorMessage.set('Verbindung zum Server verloren.');
      return;
    }
    this.reconnecting.set(true);
    const delay = Math.min(1000 * 2 ** this.reconnectAttempts, 10_000);
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => this.websocket.connect(this.wsUrl), delay);
  }

  private stopReconnectTimer(): void {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
  }

  submitAnswer(answerId: number): void {
    const question = this.currentQuestion();
    if (!question) return;
    const token = localStorage.getItem('token') ?? '';
    this.websocket.send(
      JSON.stringify({
        type: 'submit_answer',
        token,
        questionIndex: question.questionIndex,
        answerId,
      }),
    );
  }

  cleanup(): void {
    this.wantConnection = false;
    this.stopReconnectTimer();
    this.subscriptions.unsubscribe();
    this.stopCountdown();
    this.websocket.disconnect();
  }

  private handleMessage(raw: string): void {
    let msg: DuelServerMsg;
    try {
      msg = JSON.parse(raw);
    } catch {
      console.error('Ungültige WebSocket-Nachricht:', raw);
      return;
    }

    switch (msg.type) {
      case 'waiting':
        this.status.set('waiting');
        break;

      case 'game_started':
        this.host.set(msg.host);
        this.guest.set(msg.guest);
        this.totalQuestions.set(msg.totalQuestions);
        this.status.set('playing');
        break;

      case 'resumed':
        this.host.set(msg.host);
        this.guest.set(msg.guest);
        this.hostScore.set(msg.hostScore);
        this.guestScore.set(msg.guestScore);
        this.totalQuestions.set(msg.totalQuestions);
        this.status.set('playing');
        break;

      case 'question': {
        const decoded: DuelQuestionMsg = {
          ...msg,
          questionText: decodeHtml(msg.questionText),
          options: msg.options.map((o) => ({ ...o, text: decodeHtml(o.text) })),
        };
        // Sofort anzeigen: der Server pausiert selbst zwischen Ergebnis und
        // nächster Frage, und sein 5-Sekunden-Fenster läuft ab dem Senden.
        // Jede Verzögerung hier bringt die Countdown-Anzeige aus dem Takt.
        this.lastResult.set(null);
        this.currentQuestion.set(decoded);
        this.startCountdown();
        break;
      }

      case 'question_result':
        this.stopCountdown();
        this.hostScore.set(msg.hostScore);
        this.guestScore.set(msg.guestScore);
        this.lastResult.set(msg);
        this.questionResult$.next(msg);
        break;

      case 'opponent_disconnected':
        this.opponentOffline.set(true);
        break;

      case 'opponent_reconnected':
        this.opponentOffline.set(false);
        break;

      case 'game_over': {
        this.gameOverReceived = true;
        const delay = this.lastResult() !== null ? 1300 : 0;
        setTimeout(() => {
          this.gameOver.set(msg);
          this.status.set('over');
        }, delay);
        break;
      }

      case 'error':
        this.wantConnection = false; // Server-Fehler sind endgültig
        if (msg.message.toLowerCase().includes('expired')) {
          this.authService.logout();
          this.router.navigate(['/login']);
        } else {
          this.errorMessage.set(msg.message);
          console.error('Server-Fehler:', msg.message);
        }
        break;
    }
  }

  /**
   * Reine Anzeige-Uhr: das 5-Sekunden-Fenster wird vom Server durchgesetzt;
   * `question_result` beendet die Frage unabhängig von dieser Anzeige.
   */
  private startCountdown(): void {
    this.stopCountdown();
    const startedAt = Date.now();
    this.secondsLeft.set(QUESTION_SECONDS);
    this.countdownTimer = setInterval(() => {
      const left = QUESTION_SECONDS - (Date.now() - startedAt) / 1000;
      this.secondsLeft.set(Math.max(left, 0));
      if (left <= 0) this.stopCountdown();
    }, 100);
  }

  private stopCountdown(): void {
    if (this.countdownTimer) clearInterval(this.countdownTimer);
    this.countdownTimer = null;
  }

  private readUserIdFromToken(): string {
    const token = localStorage.getItem('token') ?? '';
    try {
      return JSON.parse(atob(token.split('.')[1])).id ?? '';
    } catch {
      return '';
    }
  }
}
