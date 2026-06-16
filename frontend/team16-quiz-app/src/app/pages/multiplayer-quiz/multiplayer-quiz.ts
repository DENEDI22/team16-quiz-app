import { Component, computed, effect, inject, OnDestroy, OnInit, signal } from '@angular/core';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { Subscription } from 'rxjs';
import { answerStagger, fadeIn, questionSlide } from '../../animations';
import { DuelService } from '../../services/multiplayer/duel-service';
import { LobbyService } from '../../services/multiplayer/lobby-service';

/** Mein eigenes Abschneiden bei der letzten Frage. */
type AnswerFeedback = 'correct' | 'wrong' | 'none';

@Component({
  selector: 'app-multiplayer-quiz',
  standalone: true,
  imports: [RouterLink],
  templateUrl: './multiplayer-quiz.html',
  styleUrl: './multiplayer-quiz.scss',
  animations: [questionSlide, answerStagger, fadeIn],
})
export class MultiplayerQuiz implements OnInit, OnDestroy {
  protected duel = inject(DuelService);
  private lobbyService = inject(LobbyService);
  private route = inject(ActivatedRoute);
  private router = inject(Router);

  selectedOptionId = signal<number | null>(null);
  answerFeedback = signal<AnswerFeedback | null>(null);
  correctAnswerId = signal<number | null>(null);

  /** Antworten gesperrt? Nur nach eigener Antwort oder gezeigtem Ergebnis. */
  answersLocked = computed(() => this.selectedOptionId() !== null || this.duel.lastResult() !== null);

  /**
   * Das Antwortfenster ist (laut Anzeige) vorbei, aber noch kein
   * Ergebnis da: niemand hat geantwortet, die Frage wartet auf die erste
   * Antwort ("Overtime").
   */
  overtime = computed(
    () =>
      this.duel.secondsLeft() <= 0 &&
      this.duel.lastResult() === null &&
      this.duel.currentQuestion() !== null,
  );

  statusHint = computed(() => {
    if (this.duel.lastResult() !== null) return '';
    if (this.selectedOptionId() !== null) {
      return 'Antwort gespeichert — warte auf deinen Gegner oder den Ablauf der Zeit...';
    }
    if (this.overtime()) {
      return 'Die Zeit ist um — die nächste Antwort beendet die Frage. Du kannst noch antworten!';
    }
    return '';
  });

  resultText = computed(() => {
    const result = this.duel.lastResult();
    if (!result) return '';
    const mine = this.duel.myResult();
    const theirs = this.duel.opponentResult();
    const mySummary = mine
      ? mine.correct
        ? `Richtig! +${mine.scoreDelta} Punkte`
        : 'Leider falsch.'
      : 'Du hast nicht geantwortet.';
    const theirSummary = theirs
      ? theirs.correct
        ? `richtig (+${theirs.scoreDelta})`
        : 'falsch'
      : 'keine Antwort';
    return `${mySummary} · Gegner: ${theirSummary}`;
  });

  gameOverText = computed(() => {
    const over = this.duel.gameOver();
    if (!over) return '';
    if (over.winner === null) return 'Unentschieden!';
    return over.winner === this.duel.myUserId() ? 'Du hast gewonnen! 🏆' : 'Du hast verloren.';
  });

  /** Breite der Countdown-Leiste in Prozent. */
  timeBarWidth = computed(
    () => `${(this.duel.secondsLeft() / this.duel.answerGraceSeconds()) * 100}%`,
  );

  private lobbyId = '';
  private subscriptions = new Subscription();

  constructor() {
    this.subscriptions.add(
      this.duel.questionResult$.subscribe((result) => {
        this.correctAnswerId.set(result.correctAnswerId);
        const mine = this.duel.isHost() ? result.hostResult : result.guestResult;
        this.answerFeedback.set(mine ? (mine.correct ? 'correct' : 'wrong') : 'none');
      }),
    );

    effect(() => {
      if (this.duel.currentQuestion() !== null) {
        this.answerFeedback.set(null);
        this.correctAnswerId.set(null);
        this.selectedOptionId.set(null);
      }
    });
  }

  ngOnInit(): void {
    this.lobbyId = this.route.snapshot.paramMap.get('lobbyId') ?? '';
    this.duel.connect(this.lobbyId);

    history.pushState(null, '', location.href);
    window.addEventListener('popstate', this.blockBack);
  }

  ngOnDestroy(): void {
    window.removeEventListener('popstate', this.blockBack);
    this.subscriptions.unsubscribe();
    this.duel.cleanup();
  }

  private blockBack = (): void => {
    history.pushState(null, '', location.href);
  };

  selectAnswer(optionId: number): void {
    if (this.answersLocked()) return;
    this.selectedOptionId.set(optionId);
    this.duel.submitAnswer(optionId);
  }

  /** Host bricht die Lobby ab, solange noch kein Gegner da ist. */
  cancelLobby(): void {
    this.duel.cleanup();
    this.lobbyService.deleteLobby(this.lobbyId).subscribe({
      next: () => this.router.navigate(['/multiplayer']),
      error: () => this.router.navigate(['/multiplayer']),
    });
  }
}
