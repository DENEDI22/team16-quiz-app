import { Component, inject } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { CreateLobbyRequest } from '../../models/lobby';
import { categoryOptions, difficultyOptions } from '../../models/quiz-options';

@Component({
  selector: 'app-create-lobby-dialog',
  standalone: true,
  imports: [
    FormsModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MatButtonModule,
  ],
  templateUrl: './create-lobby-dialog.html',
  styleUrl: './create-lobby-dialog.scss',
})
export class CreateLobbyDialog {
  private dialogRef = inject(MatDialogRef<CreateLobbyDialog>);

  categoryOptions = categoryOptions;
  difficultyOptions = difficultyOptions;

  name = '';
  difficulty = '';
  categories: string[] = [];
  questionCount = 20;
  answerGraceSeconds = 15;

  toggleAllCategories(): void {
    const realSelected = this.categories.filter((c) => c !== 'All');
    const allSelected = realSelected.length === categoryOptions.length;
    this.categories = allSelected ? [] : categoryOptions.map((c) => c.value);
  }

  get isValid(): boolean {
    const name = this.name.trim();
    return (
      name.length > 0 &&
      name.length <= 40 &&
      !!this.difficulty &&
      this.categories.length > 0 &&
      this.questionCount >= 10 &&
      this.questionCount <= 50 &&
      this.answerGraceSeconds >= 5 &&
      this.answerGraceSeconds <= 60
    );
  }

  create(): void {
    if (!this.isValid) return;
    const request: CreateLobbyRequest = {
      name: this.name.trim(),
      // '' = keine Einschränkung; das Backend lässt den Filter dann weg
      difficulty: this.difficulty === 'all' ? '' : this.difficulty,
      categories: this.categories.filter((c) => c !== 'All'),
      questionCount: this.questionCount,
      answerGraceSeconds: this.answerGraceSeconds,
    };
    this.dialogRef.close(request);
  }

  cancel(): void {
    this.dialogRef.close();
  }
}
