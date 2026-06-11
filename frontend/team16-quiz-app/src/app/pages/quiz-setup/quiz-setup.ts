import {Component} from '@angular/core';
import {RouterLink} from '@angular/router';
import {FormsModule} from '@angular/forms';
import {Router} from '@angular/router';
import {inject} from '@angular/core';
import {MatFormFieldModule} from '@angular/material/form-field';
import {MatSelectModule} from '@angular/material/select';
import {MatButtonModule} from '@angular/material/button';


@Component({
  selector: 'app-quiz-setup',
  standalone: true,
  imports: [
    RouterLink,
    FormsModule,
    MatFormFieldModule,
    MatSelectModule,
    MatButtonModule
  ],
  templateUrl: './quiz-setup.html',
  styleUrl: './quiz-setup.scss'
})
export class QuizSetup {

  private router = inject(Router);

  difficulty = '';
  categories: string[] = [];
  allCategories = [
    'Sports',
    'History',
    'Geography',
    'Entertainment: Music',
    'Science: Computers',
    'Entertainment: Video Games',
    'General Knowledge'
  ];

  toggleAllCategories(): void {
    const selectedRealCategories = this.categories.filter(
      category => category !== 'All'
    );

    const allSelected =
      selectedRealCategories.length === this.allCategories.length;

    if (allSelected) {
      this.categories = [];
    } else {
      this.categories = [...this.allCategories];
    }
  }

  startQuiz(): void {
    if (this.categories.length === 0 || !this.difficulty) {
      return;
    }

    this.router.navigate(['/single-player']);
  }

}
