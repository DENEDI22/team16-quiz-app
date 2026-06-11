import { Component, inject } from '@angular/core';
import { RouterLink } from '@angular/router';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatSelectModule } from '@angular/material/select';
import { MatButtonModule } from '@angular/material/button';
import { categoryOptions, difficultyOptions } from '../../models/quiz-options';

@Component({
  selector: 'app-quiz-setup',
  standalone: true,
  imports: [RouterLink, FormsModule, MatFormFieldModule, MatSelectModule, MatButtonModule],
  templateUrl: './quiz-setup.html',
  styleUrl: './quiz-setup.scss',
})
export class QuizSetup {
  private router = inject(Router);

  categoryOptions = categoryOptions;
  difficultyOptions = difficultyOptions;

  difficulty = '';
  categories: string[] = [];

  toggleAllCategories(): void {
    const realSelected = this.categories.filter((c) => c !== 'All');
    const allSelected = realSelected.length === categoryOptions.length;
    this.categories = allSelected ? [] : categoryOptions.map((c) => c.value);
  }

  startQuiz(): void {
    if (this.categories.length === 0 || !this.difficulty) {
      return;
    }

    this.router.navigate(['/single-player'], {
      state: { categories: this.categories, difficulty: this.difficulty },
    });
  }
}
