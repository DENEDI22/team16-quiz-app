import { GameMode } from './pages/game-mode/game-mode';
import { QuizSetup } from './pages/quiz-setup/quiz-setup';
import { SinglePlayerQuiz } from './pages/single-player-quiz/single-player-quiz';
import { QuizResult } from './pages/quiz-result/quiz-result';
import {Home} from './pages/home/home';
import {Routes} from '@angular/router';
import {Login} from './pages/login/login';
import {Register} from './pages/register/register';

export const routes: Routes = [
  {
    path: '',
    component: Home,
  },
  {
    path: 'game-mode',
    component: GameMode,
  },
  {
    path: 'quiz-setup',
    component: QuizSetup,
  },
  {
    path: 'single-player',
    component: SinglePlayerQuiz,
  },
  {
    path: 'quiz-result',
    component: QuizResult,
  },
  {
    path: 'login',
    component: Login,
  },
  {
    path: 'register',
    component: Register,
  },
];
