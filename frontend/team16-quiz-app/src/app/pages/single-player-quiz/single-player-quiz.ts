import { Component, computed, effect, inject, OnDestroy, OnInit, signal } from '@angular/core';
import { RouterLink } from '@angular/router';
import { Subscription } from 'rxjs';
import { getCategoryLabel, getDifficultyLabel } from '../../models/quiz-options';
import { GameService } from '../../services/quiz/game-service';
import { answerStagger, fadeIn, questionSlide } from '../../animations';

@Component({
  selector: 'app-single-player-quiz',
  standalone: true,
  imports: [RouterLink],
  templateUrl: './single-player-quiz.html',
  styleUrl: './single-player-quiz.scss',
  animations: [questionSlide, answerStagger, fadeIn],
})
export class SinglePlayerQuiz implements OnInit, OnDestroy {
  protected gameService = inject(GameService);
  selectedOptionId = signal<number | null>(null);
  answerFeedback = signal<'correct' | 'wrong' | null>(null);
  correctAnswerId = signal<number | null>(null);

  categoryLabel = computed(() => getCategoryLabel(this.gameService.categories()));
  difficultyLabel = computed(() => getDifficultyLabel(this.gameService.difficulty()));

  private subscriptions = new Subscription();

  constructor() {
    // Subject feuert synchron — kein Batching-Problem wie bei Signal-Effects
    this.subscriptions.add(
      this.gameService.answerResult$.subscribe((result) => {
        this.answerFeedback.set(result.correct ? 'correct' : 'wrong');
        this.correctAnswerId.set(result.correctAnswerId);
      }),
    );

    effect(() => {
      if (this.gameService.currentQuestion() !== null) {
        this.answerFeedback.set(null);
        this.correctAnswerId.set(null);
        this.selectedOptionId.set(null);
      }
    });
  }

  ngOnInit(): void {
    const state = window.history.state as { categories?: string[]; difficulty?: string };
    const categories = state?.categories ?? [];
    const difficulty = state?.difficulty ?? 'all';
    this.gameService.startGame('ws://localhost:7000/ws', categories, difficulty);

    history.pushState(null, '', location.href);
    window.addEventListener('popstate', this.blockBack);
  }

  ngOnDestroy(): void {
    window.removeEventListener('popstate', this.blockBack);
    this.subscriptions.unsubscribe();
    this.gameService.cleanup();
  }

  private blockBack = (): void => {
    history.pushState(null, '', location.href);
  };

  selectAnswer(optionId: number): void {
    if (this.selectedOptionId() !== null) return;
    this.selectedOptionId.set(optionId);
    this.gameService.submitAnswer(optionId);
  }
}
