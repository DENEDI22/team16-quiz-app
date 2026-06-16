import { Component, inject, OnInit, signal } from '@angular/core';
import { DatePipe, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { AccountStats, ScoreboardService } from '../../services/scoreboard/scoreboard-service';

@Component({
  selector: 'app-scoreboard-account',
  standalone: true,
  imports: [RouterLink, DatePipe, DecimalPipe],
  templateUrl: './scoreboard-account.html',
  styleUrl: './scoreboard-account.scss',
})
export class ScoreboardAccount implements OnInit {
  private scoreboard = inject(ScoreboardService);

  stats = signal<AccountStats | null>(null);
  loading = signal(true);
  error = signal(false);

  ngOnInit(): void {
    this.scoreboard.getAccountStats().subscribe({
      next: (stats) => {
        this.stats.set(stats);
        this.loading.set(false);
      },
      error: () => {
        this.error.set(true);
        this.loading.set(false);
      },
    });
  }
}
