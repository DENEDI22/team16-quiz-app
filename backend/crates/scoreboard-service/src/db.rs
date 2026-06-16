use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{
    AccountDuel, AccountStats, AnswerHistoryEntry, CategoryLeaderboardEntry, CreateAnswerRequest,
    CreateDuelResultRequest, CreateSinglePlayerResultRequest, DifficultyHighscore,
    DuelLeaderboardEntry, DuelResults, QuestionStats, SinglePlayerLeaderboard,
    SinglePlayerLeaderboardEntry, UserHighscore,
};
use crate::stats::build_question_stats;

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Fresh databases get the current schema directly. `time_to_answer_ms`
    // holds the answer duration in milliseconds (docs/api-contracts.md §1.6);
    // `category`/`difficulty` are the question's concrete values, denormalized
    // here so leaderboards can filter without joining quiz-service's DB.
    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS answers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    question uuid NOT NULL,
    user_id uuid NOT NULL,
    answer_id int NOT NULL,
    is_correct bool NOT NULL,
    timestamp timestamptz NOT NULL,
    time_to_answer_ms int NOT NULL,
    is_multiplayer bool NOT NULL,
    session_id uuid NOT NULL,
    category text NOT NULL DEFAULT 'Unknown',
    difficulty text NOT NULL DEFAULT 'Unknown',
    CONSTRAINT answers_pk PRIMARY KEY (id)
)
        ",
    )
    .execute(pool)
    .await?;

    // Migrate pre-existing databases: add the new columns if missing and
    // rename the old seconds column to milliseconds (converting the values).
    sqlx::query(
        "ALTER TABLE answers ADD COLUMN IF NOT EXISTS category text NOT NULL DEFAULT 'Unknown'",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "ALTER TABLE answers ADD COLUMN IF NOT EXISTS difficulty text NOT NULL DEFAULT 'Unknown'",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'answers' AND column_name = 'time_to_answer'
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'answers' AND column_name = 'time_to_answer_ms'
    ) THEN
        ALTER TABLE answers RENAME COLUMN time_to_answer TO time_to_answer_ms;
        UPDATE answers SET time_to_answer_ms = time_to_answer_ms * 1000;
    END IF;
END $$;
        ",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS duel_results (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    session_id uuid NOT NULL,
    host_user_id uuid NOT NULL,
    guest_user_id uuid NOT NULL,
    host_score int NOT NULL,
    guest_score int NOT NULL,
    timestamp timestamptz NOT NULL,
    CONSTRAINT duel_results_pk PRIMARY KEY (id)
)
        ",
    )
    .execute(pool)
    .await?;

    // Per-game singleplayer scores. Singleplayer never stored an aggregate
    // score before; this powers highscore-per-difficulty and the global
    // singleplayer leaderboard. `difficulty`/`categories` are the *session's*
    // selected settings (a concrete value or "All").
    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS singleplayer_results (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    session_id uuid NOT NULL,
    score int NOT NULL,
    correct_answers int NOT NULL,
    difficulty text NOT NULL,
    categories text NOT NULL,
    timestamp timestamptz NOT NULL,
    CONSTRAINT singleplayer_results_pk PRIMARY KEY (id)
)
        ",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Persists a single answer for the authenticated `user_id` (taken from the JWT,
/// not from the request body) and returns the generated record id.
pub async fn insert_answer(
    pool: &PgPool,
    user_id: Uuid,
    req: &CreateAnswerRequest,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO answers
            (question, user_id, answer_id, is_correct, timestamp, time_to_answer_ms, is_multiplayer, session_id, category, difficulty)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id",
    )
    .bind(req.question_id)
    .bind(user_id)
    .bind(req.answer_id)
    .bind(req.is_correct)
    .bind(req.timestamp)
    .bind(req.time_to_answer_ms)
    .bind(req.is_multiplayer)
    .bind(req.session_id)
    .bind(&req.category)
    .bind(&req.difficulty)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Persists a finished singleplayer game's aggregate score for the
/// authenticated `user_id`.
pub async fn insert_singleplayer_result(
    pool: &PgPool,
    user_id: Uuid,
    req: &CreateSinglePlayerResultRequest,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO singleplayer_results
            (user_id, session_id, score, correct_answers, difficulty, categories, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(user_id)
    .bind(req.session_id)
    .bind(req.score)
    .bind(req.correct_answers)
    .bind(&req.difficulty)
    .bind(&req.categories)
    .bind(req.timestamp)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn insert_duel_result(
    pool: &PgPool,
    req: &CreateDuelResultRequest,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO duel_results
            (session_id, host_user_id, guest_user_id, host_score, guest_score, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(req.session_id)
    .bind(req.host_user_id)
    .bind(req.guest_user_id)
    .bind(req.host_score)
    .bind(req.guest_score)
    .bind(req.timestamp)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn get_user_duels(pool: &PgPool, user_id: Uuid) -> Result<Vec<DuelResults>, sqlx::Error> {
    sqlx::query_as::<_, DuelResults>(
        "SELECT id, session_id, host_user_id, guest_user_id, host_score, guest_score, timestamp
         FROM duel_results
         WHERE host_user_id = $1 OR guest_user_id = $1
         ORDER BY timestamp DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_highscores(pool: &PgPool) -> Result<Vec<UserHighscore>, sqlx::Error> {
    sqlx::query_as::<_, UserHighscore>(
        "SELECT user_id,
                COUNT(*) AS total_answers,
                SUM(CASE WHEN is_correct THEN 1 ELSE 0 END) AS correct_answers
         FROM answers
         GROUP BY user_id
         ORDER BY correct_answers DESC
         LIMIT 20",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_answer_history(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<AnswerHistoryEntry>, sqlx::Error> {
    sqlx::query_as::<_, AnswerHistoryEntry>(
        "SELECT id, question, answer_id, is_correct, timestamp, time_to_answer_ms, is_multiplayer, session_id, category, difficulty
         FROM answers
         WHERE user_id = $1
         ORDER BY timestamp DESC
         LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Builds the account-overview stats for one user. Opponent usernames in the
/// returned duels are left empty here; the handler resolves them via
/// auth-service (the scoreboard DB has no user table).
pub async fn get_account_stats(pool: &PgPool, user_id: Uuid) -> Result<AccountStats, sqlx::Error> {
    let highscore_rows: Vec<(String, i32)> = sqlx::query_as(
        "SELECT difficulty, MAX(score)
         FROM singleplayer_results
         WHERE user_id = $1
         GROUP BY difficulty
         ORDER BY difficulty",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let highscores_per_difficulty = highscore_rows
        .into_iter()
        .map(|(difficulty, highscore)| DifficultyHighscore {
            difficulty,
            highscore,
        })
        .collect();

    let duel_rows: Vec<DuelResults> = sqlx::query_as(
        "SELECT id, session_id, host_user_id, guest_user_id, host_score, guest_score, timestamp
         FROM duel_results
         WHERE host_user_id = $1 OR guest_user_id = $1
         ORDER BY timestamp DESC
         LIMIT 10",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let last_duels = duel_rows
        .into_iter()
        .map(|d| {
            let is_host = d.host_user_id == user_id;
            let (own_score, opponent_score, opponent_id) = if is_host {
                (d.host_score, d.guest_score, d.guest_user_id)
            } else {
                (d.guest_score, d.host_score, d.host_user_id)
            };
            let outcome = match own_score.cmp(&opponent_score) {
                std::cmp::Ordering::Greater => "win",
                std::cmp::Ordering::Less => "loss",
                std::cmp::Ordering::Equal => "draw",
            }
            .to_string();
            AccountDuel {
                duel_id: d.id,
                session_id: d.session_id,
                opponent_id,
                opponent_username: String::new(),
                own_score,
                opponent_score,
                outcome,
                timestamp: d.timestamp,
            }
        })
        .collect();

    let avg_multiplayer_score: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(CASE WHEN host_user_id = $1 THEN host_score ELSE guest_score END)::float8
         FROM duel_results
         WHERE host_user_id = $1 OR guest_user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let duels_played: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM duel_results WHERE host_user_id = $1 OR guest_user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let avg_time_to_answer_ms: Option<f64> =
        sqlx::query_scalar("SELECT AVG(time_to_answer_ms)::float8 FROM answers WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    let wins: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM duel_results
         WHERE (host_user_id = $1 AND host_score > guest_score)
            OR (guest_user_id = $1 AND guest_score > host_score)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let win_rate = if duels_played > 0 {
        wins as f64 / duels_played as f64
    } else {
        0.0
    };

    Ok(AccountStats {
        highscores_per_difficulty,
        last_duels,
        avg_multiplayer_score: avg_multiplayer_score.unwrap_or(0.0),
        duels_played,
        avg_time_to_answer_ms: avg_time_to_answer_ms.unwrap_or(0.0),
        win_rate,
    })
}

/// Top-10 players by number of duels won (strictly higher score; draws count
/// for nobody). Usernames are filled in by the handler.
pub async fn get_duel_leaderboard(pool: &PgPool) -> Result<Vec<DuelLeaderboardEntry>, sqlx::Error> {
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT winner, COUNT(*) AS duels_won
         FROM (
             SELECT CASE
                 WHEN host_score > guest_score THEN host_user_id
                 WHEN guest_score > host_score THEN guest_user_id
                 ELSE NULL
             END AS winner
             FROM duel_results
         ) w
         WHERE winner IS NOT NULL
         GROUP BY winner
         ORDER BY duels_won DESC
         LIMIT 10",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(user_id, duels_won)| DuelLeaderboardEntry {
            user_id,
            username: String::new(),
            duels_won,
        })
        .collect())
}

/// Top-10 singleplayer highscores per difficulty bucket. Usernames are filled
/// in by the handler.
pub async fn get_singleplayer_leaderboard(
    pool: &PgPool,
) -> Result<Vec<SinglePlayerLeaderboard>, sqlx::Error> {
    let rows: Vec<(String, Uuid, i32)> = sqlx::query_as(
        "SELECT difficulty, user_id, highscore
         FROM (
             SELECT difficulty, user_id, MAX(score) AS highscore,
                    ROW_NUMBER() OVER (PARTITION BY difficulty ORDER BY MAX(score) DESC) AS rn
             FROM singleplayer_results
             GROUP BY difficulty, user_id
         ) ranked
         WHERE rn <= 10
         ORDER BY difficulty, highscore DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut boards: Vec<SinglePlayerLeaderboard> = Vec::new();
    for (difficulty, user_id, highscore) in rows {
        let entry = SinglePlayerLeaderboardEntry {
            user_id,
            username: String::new(),
            highscore,
        };
        match boards.last_mut() {
            Some(board) if board.difficulty == difficulty => board.entries.push(entry),
            _ => boards.push(SinglePlayerLeaderboard {
                difficulty,
                entries: vec![entry],
            }),
        }
    }
    Ok(boards)
}

/// Top-10 players by accuracy in a specific category, among those who have
/// answered at least `min_answers` questions in it. Usernames filled in by the
/// handler.
pub async fn get_category_leaderboard(
    pool: &PgPool,
    category: &str,
    min_answers: i64,
) -> Result<Vec<CategoryLeaderboardEntry>, sqlx::Error> {
    let rows: Vec<(Uuid, i64, i64, f64)> = sqlx::query_as(
        "SELECT user_id,
                COUNT(*) AS total_answers,
                SUM(CASE WHEN is_correct THEN 1 ELSE 0 END) AS correct_answers,
                (SUM(CASE WHEN is_correct THEN 1 ELSE 0 END)::float8 / COUNT(*)) AS accuracy
         FROM answers
         WHERE category = $1
         GROUP BY user_id
         HAVING COUNT(*) >= $2
         ORDER BY accuracy DESC, correct_answers DESC
         LIMIT 10",
    )
    .bind(category)
    .bind(min_answers)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(user_id, total_answers, correct_answers, accuracy)| CategoryLeaderboardEntry {
                user_id,
                username: String::new(),
                total_answers,
                correct_answers,
                accuracy,
            },
        )
        .collect())
}

pub async fn get_question_stats(
    pool: &PgPool,
    question_id: Uuid,
) -> Result<Option<QuestionStats>, sqlx::Error> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers WHERE question = $1")
        .bind(question_id)
        .fetch_one(pool)
        .await?;

    if total == 0 {
        return Ok(None);
    }

    let counts: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT answer_id, COUNT(*)
         FROM answers
         WHERE question = $1
         GROUP BY answer_id
         ORDER BY answer_id",
    )
    .bind(question_id)
    .fetch_all(pool)
    .await?;

    let correct_answer_id: Option<i32> = sqlx::query_scalar(
        "SELECT answer_id FROM answers WHERE question = $1 AND is_correct = true LIMIT 1",
    )
    .bind(question_id)
    .fetch_optional(pool)
    .await?;

    Ok(Some(build_question_stats(
        question_id,
        total,
        &counts,
        correct_answer_id,
    )))
}
