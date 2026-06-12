import { AnswerOption } from './questions';

export interface PlayerInfo {
  id: string;
  /** Öffentlicher Anzeigename — niemals die E-Mail anderer Nutzer anzeigen. */
  username: string;
}

export interface LobbySettings {
  /** '' = alle Schwierigkeiten */
  difficulty: string;
  /** [] = alle Kategorien */
  categories: string[];
  questionCount: number;
}

export interface Lobby {
  id: string;
  name: string;
  host: PlayerInfo;
  guest?: PlayerInfo;
  settings: LobbySettings;
  status: 'waiting' | 'full';
  createdAt: string;
}

export interface CreateLobbyRequest {
  name: string;
  difficulty: string;
  categories: string[];
  questionCount: number;
}

// ── Duell-WebSocket-Nachrichten (multiplayer-service) ───────────────────────

export interface DuelWaitingMsg {
  type: 'waiting';
}

export interface DuelGameStartedMsg {
  type: 'game_started';
  sessionId: string;
  host: PlayerInfo;
  guest: PlayerInfo;
  totalQuestions: number;
}

export interface DuelQuestionMsg {
  type: 'question';
  questionIndex: number;
  questionId: string;
  questionText: string;
  options: AnswerOption[];
}

export interface PlayerAnswerResult {
  answerId: number;
  correct: boolean;
  scoreDelta: number;
}

export interface DuelQuestionResultMsg {
  type: 'question_result';
  questionIndex: number;
  correctAnswerId: number;
  /** null = dieser Spieler hat nicht geantwortet */
  hostResult: PlayerAnswerResult | null;
  guestResult: PlayerAnswerResult | null;
  hostScore: number;
  guestScore: number;
}

export interface DuelResumedMsg {
  type: 'resumed';
  sessionId: string;
  host: PlayerInfo;
  guest: PlayerInfo;
  hostScore: number;
  guestScore: number;
  questionIndex: number;
  totalQuestions: number;
}

export interface DuelOpponentDisconnectedMsg {
  type: 'opponent_disconnected';
}

export interface DuelOpponentReconnectedMsg {
  type: 'opponent_reconnected';
}

export interface DuelGameOverMsg {
  type: 'game_over';
  hostScore: number;
  guestScore: number;
  /** null = Unentschieden */
  winner: string | null;
}

export interface DuelErrorMsg {
  type: 'error';
  message: string;
}

export type DuelServerMsg =
  | DuelWaitingMsg
  | DuelGameStartedMsg
  | DuelQuestionMsg
  | DuelQuestionResultMsg
  | DuelResumedMsg
  | DuelOpponentDisconnectedMsg
  | DuelOpponentReconnectedMsg
  | DuelGameOverMsg
  | DuelErrorMsg;
