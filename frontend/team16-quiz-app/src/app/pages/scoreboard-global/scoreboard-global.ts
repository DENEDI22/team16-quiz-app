import { Component, inject, OnInit, signal } from '@angular/core';
import { DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import {
  CategoryLeaderboardEntry,
  DuelLeaderboardEntry,
  ScoreboardService,
  SinglePlayerLeaderboard,
} from '../../services/scoreboard/scoreboard-service';

type Tab = 'duels' | 'singleplayer' | 'category';

@Component({
  selector: 'app-scoreboard-global',
  standalone: true,
  imports: [RouterLink, DecimalPipe],
  templateUrl: './scoreboard-global.html',
  styleUrl: './scoreboard-global.scss',
})
export class ScoreboardGlobal implements OnInit {
  private scoreboard = inject(ScoreboardService);

  /** Matches the singleplayer category choices (quiz-setup/categories.ts). */
  readonly categories = [
    'Sports',
    'History',
    'Geography',
    'Entertainment: Music',
    'Science: Computers',
    'Entertainment: Video Games',
    'General Knowledge',
  ];

  activeTab = signal<Tab>('duels');

  duels = signal<DuelLeaderboardEntry[]>([]);
  singleplayer = signal<SinglePlayerLeaderboard[]>([]);
  categoryEntries = signal<CategoryLeaderboardEntry[]>([]);
  selectedCategory = signal<string>('General Knowledge');

  loading = signal(false);
  error = signal(false);

  ngOnInit(): void {
    this.loadDuels();
    this.loadSingleplayer();
  }

  selectTab(tab: Tab): void {
    this.activeTab.set(tab);
    if (tab === 'category' && this.categoryEntries().length === 0) {
      this.loadCategory();
    }
  }

  onCategoryChange(category: string): void {
    this.selectedCategory.set(category);
    this.loadCategory();
  }

  private loadDuels(): void {
    this.scoreboard.getDuelLeaderboard().subscribe({
      next: (entries) => this.duels.set(entries),
      error: () => this.error.set(true),
    });
  }

  private loadSingleplayer(): void {
    this.scoreboard.getSinglePlayerLeaderboard().subscribe({
      next: (boards) => this.singleplayer.set(boards),
      error: () => this.error.set(true),
    });
  }

  private loadCategory(): void {
    this.loading.set(true);
    this.scoreboard.getCategoryLeaderboard(this.selectedCategory()).subscribe({
      next: (entries) => {
        this.categoryEntries.set(entries);
        this.loading.set(false);
      },
      error: () => {
        this.error.set(true);
        this.loading.set(false);
      },
    });
  }
}
