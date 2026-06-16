import { GameMode } from './pages/game-mode/game-mode';
import { QuizSetup } from './pages/quiz-setup/quiz-setup';
import { SinglePlayerQuiz } from './pages/single-player-quiz/single-player-quiz';
import { QuizResult } from './pages/quiz-result/quiz-result';
import { MultiplayerLobbies } from './pages/multiplayer-lobbies/multiplayer-lobbies';
import { MultiplayerQuiz } from './pages/multiplayer-quiz/multiplayer-quiz';
import { ScoreboardAccount } from './pages/scoreboard-account/scoreboard-account';
import { ScoreboardGlobal } from './pages/scoreboard-global/scoreboard-global';
import {Home} from './pages/home/home';
import {Routes} from '@angular/router';
import {Login} from './pages/login/login';
import {Register} from './pages/register/register';
import { authGuard} from './auth/guards/auth-guard';
import { Datenschutz } from './pages/datenschutz/datenschutz/datenschutz';
import { Agb } from './pages/agb/agb/agb';

export const routes: Routes = [
  {
    path: '',
    component: Home,
  },
  {
    path: 'game-mode',
    component: GameMode,
    canActivate: [authGuard],
  },
  {
    path: 'quiz-setup',
    component: QuizSetup,
    canActivate: [authGuard],
  },
  {
    path: 'single-player',
    component: SinglePlayerQuiz,
    canActivate: [authGuard],
  },
  {
    path: 'quiz-result',
    component: QuizResult,
    canActivate: [authGuard],
  },
  {
    path: 'multiplayer',
    component: MultiplayerLobbies,
    canActivate: [authGuard],
  },
  {
    path: 'multiplayer/duel/:lobbyId',
    component: MultiplayerQuiz,
    canActivate: [authGuard],
  },
  {
    path: 'scoreboard/account',
    component: ScoreboardAccount,
    canActivate: [authGuard],
  },
  {
    path: 'scoreboard/global',
    component: ScoreboardGlobal,
    canActivate: [authGuard],
  },
  {
    path: 'login',
    component: Login,
  },
  {
    path: 'register',
    component: Register,
  },
  {
    path: 'datenschutz',
    component: Datenschutz,
  },
  {
    path: 'agb',
    component: Agb,
  }
];
