import { Component, inject, OnDestroy, OnInit, signal } from '@angular/core';
import { Router, RouterLink } from '@angular/router';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog, MatDialogModule } from '@angular/material/dialog';
import { CreateLobbyRequest, Lobby } from '../../models/lobby';
import { getCategoryLabel, getDifficultyLabel } from '../../models/quiz-options';
import { LobbyService } from '../../services/multiplayer/lobby-service';
import { fadeIn } from '../../animations';
import { CreateLobbyDialog } from './create-lobby-dialog';

const REFRESH_INTERVAL_MS = 5000;

@Component({
  selector: 'app-multiplayer-lobbies',
  standalone: true,
  imports: [RouterLink, MatButtonModule, MatDialogModule],
  templateUrl: './multiplayer-lobbies.html',
  styleUrl: './multiplayer-lobbies.scss',
  animations: [fadeIn],
})
export class MultiplayerLobbies implements OnInit, OnDestroy {
  private lobbyService = inject(LobbyService);
  private router = inject(Router);
  private dialog = inject(MatDialog);

  lobbies = signal<Lobby[]>([]);
  isLoading = signal(true);
  errorMessage = signal('');
  joiningId = signal<string | null>(null);

  private refreshTimer: ReturnType<typeof setInterval> | null = null;

  ngOnInit(): void {
    this.loadLobbies();
    this.refreshTimer = setInterval(() => this.loadLobbies(), REFRESH_INTERVAL_MS);
  }

  ngOnDestroy(): void {
    if (this.refreshTimer) clearInterval(this.refreshTimer);
  }

  loadLobbies(): void {
    this.lobbyService.getLobbies().subscribe({
      next: (lobbies) => {
        this.lobbies.set(lobbies);
        this.isLoading.set(false);
      },
      error: () => {
        this.errorMessage.set('Lobbys konnten nicht geladen werden.');
        this.isLoading.set(false);
      },
    });
  }

  openCreateDialog(): void {
    const ref = this.dialog.open(CreateLobbyDialog, {
      width: '480px',
      autoFocus: 'first-tabbable',
    });
    ref.afterClosed().subscribe((request?: CreateLobbyRequest) => {
      if (!request) return;
      this.errorMessage.set('');
      this.lobbyService.createLobby(request).subscribe({
        next: (lobby) => this.router.navigate(['/multiplayer/duel', lobby.id]),
        error: (err) =>
          this.errorMessage.set(err.error?.message ?? 'Lobby konnte nicht erstellt werden.'),
      });
    });
  }

  joinLobby(lobby: Lobby): void {
    if (this.joiningId() !== null) return;
    this.joiningId.set(lobby.id);
    this.errorMessage.set('');
    this.lobbyService.joinLobby(lobby.id).subscribe({
      next: () => this.router.navigate(['/multiplayer/duel', lobby.id]),
      error: (err) => {
        this.joiningId.set(null);
        this.errorMessage.set(err.error?.message ?? 'Beitritt fehlgeschlagen.');
        // Wahrscheinlich war die Lobby inzwischen voll oder weg: Liste auffrischen.
        this.loadLobbies();
      },
    });
  }

  difficultyLabel(lobby: Lobby): string {
    return lobby.settings.difficulty === '' ? 'Alle' : getDifficultyLabel(lobby.settings.difficulty);
  }

  categoryLabel(lobby: Lobby): string {
    return getCategoryLabel(lobby.settings.categories);
  }
}
