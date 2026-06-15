export interface AnswerOption {
  id: number;
  text: string;
}

export interface GameStartedMsg {
  type: 'game_started';
  sessionId: string;
  livesRemaining: number;
}

export interface QuestionMsg {
  type: 'question';
  questionId: string;
  questionText: string;
  options: AnswerOption[];
  questionIndex: number;
}

export interface AnswerResultMsg {
  type: 'answer_result';
  correct: boolean;
  correctAnswerId: number;
  totalScore: number;
  livesRemaining: number;
}

export interface GameOverMsg {
  type: 'game_over';
  totalScore: number;
  correctAnswers: number;
}

export interface ErrorMsg {
  type: 'error';
  message: string;
}

export type ServerMsg = GameStartedMsg | QuestionMsg | AnswerResultMsg | GameOverMsg | ErrorMsg;
