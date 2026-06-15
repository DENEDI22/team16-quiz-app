import { inject, Injectable, signal } from '@angular/core';
import { Router } from '@angular/router';
import { Subject, Subscription } from 'rxjs';
import { AnswerResultMsg, GameOverMsg, QuestionMsg, ServerMsg } from '../../models/questions';
import { AuthService } from '../auth/auth';
import { WebsocketService } from '../ws/WebsocketService';

function decodeHtml(html: string): string {
  const txt = document.createElement('textarea');
  txt.innerHTML = html;
  return txt.value;
}

@Injectable({
  providedIn: 'root',
})
export class GameService {
  private websocket = inject(WebsocketService);
  private authService = inject(AuthService);
  private router = inject(Router);
  private subscriptions = new Subscription();
  private questionStartTime = 0;

  lives = signal(0);
  score = signal(0);
  currentQuestion = signal<QuestionMsg | null>(null);
  lastResult = signal<AnswerResultMsg | null>(null);
  gameOver = signal<GameOverMsg | null>(null);
  categories = signal<string[]>([]);
  difficulty = signal<string>('');

  readonly answerResult$ = new Subject<AnswerResultMsg>();

  startGame(baseUrl: string, categories: string[], difficulty: string): void {
    this.subscriptions.unsubscribe();
    this.subscriptions = new Subscription();

    this.lives.set(0);
    this.score.set(0);
    this.currentQuestion.set(null);
    this.lastResult.set(null);
    this.gameOver.set(null);
    this.categories.set(categories);
    this.difficulty.set(difficulty);

    this.subscriptions.add(
      this.websocket.connected$.subscribe(() => {
        const token = localStorage.getItem('token') ?? '';
        this.websocket.send(JSON.stringify({ type: 'start_game', token }));
      }),
    );

    this.subscriptions.add(this.websocket.messages$.subscribe((raw) => this.handleMessage(raw)));

    const wsUrl = this.buildWsUrl(baseUrl, categories, difficulty);
    console.log('[GameService] WebSocket URL:', wsUrl);
    this.websocket.connect(wsUrl);
  }

  private buildWsUrl(baseUrl: string, categories: string[], difficulty: string): string {
    const params = new URLSearchParams();
    const realCategories = categories.filter((c) => c !== 'All');
    if (realCategories.length > 0) {
      params.set('categories', realCategories.join(','));
    }
    if (difficulty && difficulty !== 'all') {
      params.set('difficulty', difficulty);
    }
    const query = params.toString();
    return query ? `${baseUrl}?${query}` : baseUrl;
  }

  submitAnswer(answerId: number): void {
    const question = this.currentQuestion();
    if (!question) return;
    const token = localStorage.getItem('token') ?? '';
    const elapsedMs = Date.now() - this.questionStartTime;
    this.websocket.send(
      JSON.stringify({
        type: 'submit_answer',
        token,
        questionId: question.questionId,
        answerId,
        timeToAnswerMs: elapsedMs,
      }),
    );
  }

  cleanup(): void {
    this.subscriptions.unsubscribe();
    this.websocket.disconnect();
  }

  private handleMessage(raw: string): void {
    let msg: ServerMsg;
    try {
      msg = JSON.parse(raw);
    } catch {
      console.error('Ungültige WebSocket-Nachricht:', raw);
      return;
    }

    switch (msg.type) {
      case 'game_started':
        this.lives.set(msg.livesRemaining);
        break;
      case 'question': {
        const decoded = {
          ...msg,
          questionText: decodeHtml(msg.questionText),
          options: msg.options.map((o) => ({ ...o, text: decodeHtml(o.text) })),
        };
        const delay = this.lastResult() !== null ? 800 : 0;
        this.lastResult.set(null);
        setTimeout(() => {
          this.currentQuestion.set(decoded);
          this.questionStartTime = Date.now();
        }, delay);
        break;
      }
      case 'answer_result':
        this.answerResult$.next(msg);
        this.lastResult.set(msg);
        this.lives.set(msg.livesRemaining);
        this.score.set(msg.totalScore);
        break;
      case 'game_over': {
        const goDelay = this.lastResult() !== null ? 1300 : 0;
        setTimeout(() => this.gameOver.set(msg), goDelay);
        break;
      }
      case 'error':
        if (msg.message.toLowerCase().includes('expired')) {
          this.authService.logout();
          this.router.navigate(['/login']);
        } else {
          console.error('Server-Fehler:', msg.message);
        }
        break;
    }
  }
}
