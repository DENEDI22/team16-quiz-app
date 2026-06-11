import { Component } from '@angular/core';
import { RouterLink } from '@angular/router';

@Component({
  selector: 'app-game-mode',
  standalone: true,
  imports: [RouterLink],
  templateUrl: './game-mode.html',
  styleUrl: './game-mode.scss'
})
export class GameMode {
}
