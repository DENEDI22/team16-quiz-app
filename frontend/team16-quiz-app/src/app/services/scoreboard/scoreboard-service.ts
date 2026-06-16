import { inject, Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';

/** Standard backend envelope (docs/api-contracts.md §1.4). */
interface Envelope<T> {
  success: boolean;
  data: T;
}

export interface DifficultyHighscore {
  difficulty: string;
  highscore: number;
}

export interface AccountDuel {
  duelId: string;
  sessionId: string;
  opponentId: string;
  opponentUsername: string;
  ownScore: number;
  opponentScore: number;
  outcome: 'win' | 'loss' | 'draw';
  timestamp: string;
}

export interface AccountStats {
  highscoresPerDifficulty: DifficultyHighscore[];
  lastDuels: AccountDuel[];
  avgMultiplayerScore: number;
  duelsPlayed: number;
  avgTimeToAnswerMs: number;
  winRate: number;
}

export interface DuelLeaderboardEntry {
  userId: string;
  username: string;
  duelsWon: number;
}

export interface SinglePlayerLeaderboardEntry {
  userId: string;
  username: string;
  highscore: number;
}

export interface SinglePlayerLeaderboard {
  difficulty: string;
  entries: SinglePlayerLeaderboardEntry[];
}

export interface CategoryLeaderboardEntry {
  userId: string;
  username: string;
  totalAnswers: number;
  correctAnswers: number;
  /** 0.0–1.0 */
  accuracy: number;
}

/**
 * Reads pre-aggregated stats from scoreboard-service. The frontend performs no
 * calculations itself (Req 2) — it only renders what the backend returns.
 */
@Injectable({
  providedIn: 'root',
})
export class ScoreboardService {
  private http = inject(HttpClient);
  private apiUrl = '/api/scoreboard';

  getAccountStats(): Observable<AccountStats> {
    return this.http
      .get<Envelope<AccountStats>>(`${this.apiUrl}/account-stats`)
      .pipe(map((res) => res.data));
  }

  getDuelLeaderboard(): Observable<DuelLeaderboardEntry[]> {
    return this.http
      .get<Envelope<DuelLeaderboardEntry[]>>(`${this.apiUrl}/leaderboard/duels`)
      .pipe(map((res) => res.data));
  }

  getSinglePlayerLeaderboard(): Observable<SinglePlayerLeaderboard[]> {
    return this.http
      .get<Envelope<SinglePlayerLeaderboard[]>>(`${this.apiUrl}/leaderboard/singleplayer`)
      .pipe(map((res) => res.data));
  }

  getCategoryLeaderboard(category: string): Observable<CategoryLeaderboardEntry[]> {
    return this.http
      .get<Envelope<CategoryLeaderboardEntry[]>>(`${this.apiUrl}/leaderboard/category`, {
        params: { category },
      })
      .pipe(map((res) => res.data));
  }
}
