import { Component, signal } from '@angular/core';
import { RouterLink } from '@angular/router';

@Component({
  selector: 'app-single-player-quiz',
  standalone: true,
  imports: [RouterLink],
  templateUrl: './single-player-quiz.html',
  styleUrl: './single-player-quiz.scss'
})
export class SinglePlayerQuiz {
  lives = signal(3);
  score = signal(0);
  selectedAnswer = signal('');

  question = signal({
    category: 'Sport',
    difficulty: 'Einfach',
    text: 'In welchem Sport wird ein Shuttlecock verwendet?',
    answers: [
      'Badminton',
      'Rugby',
      'Cricket',
      'Tischtennis'
    ],
    correctAnswer: 'Badminton'
  });

  selectAnswer(answer: string): void {
    this.selectedAnswer.set(answer);

    if (answer === this.question().correctAnswer) {
      this.score.update(score => score + 1);
    } else {
      this.lives.update(lives => lives - 1);
    }
  }
}
