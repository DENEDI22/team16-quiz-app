import { Component, computed, effect, inject, OnDestroy, OnInit, signal } from '@angular/core';
import { RouterLink } from '@angular/router';
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

  categoryLabel = computed(() => getCategoryLabel(this.gameService.categories()));
  difficultyLabel = computed(() => getDifficultyLabel(this.gameService.difficulty()));

  constructor() {
    effect(() => {
      const result = this.gameService.lastResult();
      if (result !== null) {
        this.answerFeedback.set(result.correct ? 'correct' : 'wrong');
      }
    });

    effect(() => {
      if (this.gameService.currentQuestion() !== null) {
        this.answerFeedback.set(null);
        this.selectedOptionId.set(null);
      }
    });
  }

  ngOnInit(): void {
    const state = window.history.state as { categories?: string[]; difficulty?: string };
    const categories = state?.categories ?? [];
    const difficulty = state?.difficulty ?? 'all';
    this.gameService.startGame('ws://localhost:7000/ws', categories, difficulty);
  }

  ngOnDestroy(): void {
    this.gameService.cleanup();
  }

  selectAnswer(optionId: number): void {
    if (this.selectedOptionId() !== null) return;
    this.selectedOptionId.set(optionId);
    this.gameService.submitAnswer(optionId);
  }
}
