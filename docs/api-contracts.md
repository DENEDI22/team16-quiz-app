# API Contracts — team16-quiz-app backend

Status: **agreed design, not yet implemented.** This document is the single source of
truth for data formats between the four backend services (and the frontend). Where
current code disagrees with this document, the code is wrong — see
[Required changes per service](#required-changes-per-service).

Services:

| Service | Container port | Talks to |
|---|---|---|
| auth-service | 3000 | — |
| quiz-service | 3000 | OpenTDB (external) |
| scoreboard-service | 3000 | — |
| singleplayer-service | 3000 | quiz-service, scoreboard-service |

---

## 1. Global conventions

### 1.1 Entity IDs are UUIDs

Every entity is identified by a UUID (v4, serialized as the canonical lowercase
hyphenated string, e.g. `"550e8400-e29b-41d4-a716-446655440000"`):

| Entity | Owned by | Notes |
|---|---|---|
| user | auth-service | already UUID |
| question | quiz-service | **changes** from `SERIAL` (i32) to `uuid` |
| answer record | scoreboard-service | already UUID |
| duel | scoreboard-service | already UUID |
| session | singleplayer-service | full `Uuid::new_v4()`, **no** `sess_` prefix / truncation |

**Answer options are NOT entities.** An answer option is identified by its 1-based
integer index within its question (see §1.2). Never UUID, never a prefixed string
like `"a_1"`.

### 1.2 Canonical answer-option order

For a question, the option list is: all four option texts (`correctAnswer` +
`incorrectAnswers`) sorted lexicographically by Unicode code point, ascending.
`answerId` / `correctAnswerId` is the **1-based index** into that sorted list
(type: integer, 1–4). Every service derives the same index from the same
question data; no service may shuffle.

### 1.3 JSON field naming

All JSON fields are **camelCase** (`questionId`, `incorrectAnswers`,
`timeToAnswerMs`). In Rust: `#[serde(rename_all = "camelCase")]` on every
request/response struct.

### 1.4 Response envelope

Every HTTP endpoint (except `/health`) wraps its response:

```jsonc
// 2xx
{ "success": true, "data": { /* payload, may be an object or array */ } }

// 4xx / 5xx
{ "success": false, "error": { "message": "human-readable reason" } }
```

Status codes carry the semantics: `200` read OK, `201` created, `401` missing or
invalid token, `403` valid token but insufficient role, `404` not found,
`409` conflict, `422` body failed validation, `500` internal error.

### 1.5 Health endpoints

`GET /health` on every service, **no auth, no envelope**:

```json
{ "status": "healthy" }
```

### 1.6 Timestamps and durations

- Timestamps: RFC 3339 / ISO 8601 strings in UTC, e.g. `"2026-06-11T14:30:00Z"`
  (`chrono::DateTime<Utc>` default serde format).
- Durations: integer **seconds**, field names suffixed `Seconds`
  (`timeToAnswerSeconds`). Type: `i32` on the wire and in the DB.

---

## 2. Authentication

### 2.1 Model

Every endpoint except `/health`, `/login`, `/register`, `/refresh` requires:

```
Authorization: Bearer <accessToken>
```

There are **no service accounts**. When service A calls service B on behalf of a
user, A forwards the user's own token unchanged (token pass-through). All
services share `JWT_SECRET` and validate tokens locally with `shared::jwt`.

### 2.2 JWT claims

```jsonc
{
  "id":    "<user uuid>",
  "email": "user@example.com",
  "role":  "User" | "Admin",
  "exp":   1765465200          // unix seconds
}
```

- Access-token TTL: **15 minutes**.
- The `user_id` stored by any service MUST come from the token claims, never
  from a request body.

### 2.3 Refresh flow (stateless re-issue)

`POST /refresh` on auth-service. The client presents its current access token
(valid **or expired for less than 60 minutes**) in the `Authorization` header;
auth-service verifies the signature, ignores `exp` within that grace window, and
returns a newly signed token with the same claims and a fresh `exp`.

- No refresh-token table, no revocation; logout is client-side token deletion.
  (Accepted trade-off; revisit if revocation is ever needed.)
- Tokens expired beyond the grace window → `401`, client must log in again.
- Frontend behavior: on any `401` from any service, call `/refresh` once and
  retry; if refresh also fails, redirect to login.

### 2.4 WebSocket auth (singleplayer)

A browser WebSocket cannot set headers, so:

- `start_game` carries the access token; singleplayer validates it and takes
  `userId` from the claims (the client never sends a bare `userId`).
- Every `submit_answer` also carries the client's **current** token. Singleplayer
  always forwards the most recently received valid token to scoreboard-service,
  so a refresh mid-game propagates automatically.

---

## 3. auth-service

### POST /register — no auth

Request:

```json
{ "email": "a@b.com", "password": "secret", "username": "alice" }
```

Responses: `201` `{ "success": true, "data": { "token": "<jwt>" } }` ·
`409` email already registered.

### POST /login — no auth

Request:

```json
{ "email": "a@b.com", "password": "secret" }
```

Responses: `200` `{ "success": true, "data": { "token": "<jwt>" } }` ·
`401` invalid credentials.

> Note: `/register` and `/login` return the **identical** success shape. (Fixes
> the current split where login hides the token in `data.message`.)

### POST /refresh — expired-token tolerant (§2.3)

Headers: `Authorization: Bearer <token>` (valid or ≤60 min expired).
Responses: `200` `{ "success": true, "data": { "token": "<jwt>" } }` · `401`.

### GET /me — auth

`200` `{ "success": true, "data": { "id": "<uuid>", "email": "…", "role": "User" } }`

---

## 4. quiz-service

All endpoints require auth (§2.1).

### GET /questions — auth (any role)

Returns one random question.

```jsonc
// 200
{
  "success": true,
  "data": {
    "questionId": "<uuid>",
    "category": "Science: Computers",
    "difficulty": "medium",            // "easy" | "medium" | "hard"
    "question": "What does CPU stand for?",
    "correctAnswer": "Central Processing Unit",
    "incorrectAnswers": ["Central Process Unit", "Computer Personal Unit", "Central Processor Unit"]
  }
}
```

`404` if the questions table is empty.

> `questionId` is never optional and never derived (no hash fallback in
> consumers). DB schema: `id uuid DEFAULT gen_random_uuid() PRIMARY KEY`.

### POST /scrape — auth, **Admin role only**

Triggers a manual OpenTDB scrape. `200`
`{ "success": true, "data": { "message": "Scrape triggered" } }` · `403` non-admin.

---

## 5. scoreboard-service

All endpoints require auth (§2.1).

### POST /post-answer — auth

Records one answer for the **authenticated** user (`userId` is taken from the
token; it does not appear in the body).

Request:

```jsonc
{
  "questionId": "<uuid>",
  "answerId": 3,                          // 1-based option index, §1.2
  "isCorrect": true,
  "timestamp": "2026-06-11T14:30:00Z",
  "timeToAnswerSeconds": 4,
  "isMultiplayer": false,
  "sessionId": "<uuid>"
}
```

Response: `201` `{ "success": true, "data": { "answerRecordId": "<uuid>" } }`

### POST /duel-results — auth

Request:

```jsonc
{
  "sessionId": "<uuid>",
  "hostUserId": "<uuid>",
  "guestUserId": "<uuid>",
  "hostScore": 300,
  "guestScore": 200,
  "timestamp": "2026-06-11T14:30:00Z"
}
```

Response: `201` `{ "success": true, "data": { "duelId": "<uuid>" } }`

### GET /user-duels?userId=\<uuid\> — auth

`200` `{ "success": true, "data": [ { "duelId": "…", "sessionId": "…", "hostUserId": "…", "guestUserId": "…", "hostScore": 300, "guestScore": 200, "timestamp": "…" } ] }`

### GET /question-stats?questionId=\<uuid\> — auth

```jsonc
// 200
{
  "success": true,
  "data": {
    "questionId": "<uuid>",
    "totalAnswers": 40,
    "questionType": "Multiple",          // "Multiple" | "TrueFalse"
    "correctAnswerId": 2,                // integer index; 0 = unknown
    "options": [
      { "answerId": 1, "percentage": 25.0 },
      { "answerId": 2, "percentage": 75.0 }
    ]
  }
}
```

`404` if no answers recorded. `answerId`/`correctAnswerId` are integers
(currently stringified ints — changes).

---

## 6. singleplayer-service (WebSocket)

`GET /ws` upgrades to a WebSocket. All messages are JSON with a `type` tag.

### Client → server

```jsonc
{ "type": "start_game", "token": "<jwt>" }

{
  "type": "submit_answer",
  "token": "<jwt>",                 // client's CURRENT token, §2.4
  "questionId": "<uuid>",
  "answerId": 3,                    // integer index, §1.2
  "timeToAnswerSeconds": 4
}
```

### Server → client

```jsonc
{ "type": "game_started", "sessionId": "<uuid>", "livesRemaining": 3 }

{
  "type": "question",
  "questionId": "<uuid>",
  "questionText": "What does CPU stand for?",
  "options": [ { "id": 1, "text": "Central Process Unit" }, { "id": 2, "text": "Central Processing Unit" } /* … */ ],
  "questionIndex": 1
}

{ "type": "answer_result", "correct": true, "correctAnswerId": 2, "totalScore": 100, "livesRemaining": 3 }

{ "type": "game_over", "totalScore": 300, "correctAnswers": 3 }

{ "type": "error", "message": "expected start_game message" }
```

### Outbound calls

| Call | Contract | Auth |
|---|---|---|
| `GET {QUIZ_SERVICE_URL}/questions` | §4 | forward user token |
| `POST {SCOREBOARD_SERVICE_URL}/post-answer` | §5 | forward user token |

Non-2xx responses from either service MUST be logged with status and body
(no silent fire-and-forget).

---

## 7. Required changes per service

### auth-service
- [ ] `/login` success shape → `{ "success": true, "data": { "token": … } }` (same as register)
- [ ] Register returns `201` (currently `200`)
- [ ] Error shape → `{ "success": false, "error": { "message": … } }` (currently `data.message`)
- [ ] Add `POST /refresh` (signature-valid token, `exp` grace ≤ 60 min → re-issue)
- [ ] Access-token TTL 10 → 15 min
- [ ] `/health` → `{ "status": "healthy" }` JSON (currently plain text)

### quiz-service
- [ ] `questions.id`: `SERIAL` → `uuid DEFAULT gen_random_uuid()` (dev DB: drop & recreate)
- [ ] Response fields → camelCase (`correctAnswer`, `incorrectAnswers`), `id` → `questionId`
- [ ] Error shape → standard envelope (currently `{success, message}` flat)
- [ ] Require auth on `/questions` (any role) and `/scrape` (Admin); needs `JWT_SECRET`
- [ ] Add `shared` dependency for the `Auth` extractor

### scoreboard-service
- [ ] `CreateAnswerRequest.time_to_answer` field name → `timeToAnswerSeconds`
- [ ] Success shapes → standard envelope (`{status:"ok",…}` and bare arrays today)
- [ ] Error shape → standard envelope
- [ ] `question-stats`: `answerId`/`correctAnswerId` as integers (currently stringified)
- [ ] Read `JWT_SECRET` from `AppState`, not `var()` per request (cleanup, not contract)

### singleplayer-service
- [ ] `start_game`: take `token` instead of `userId`; validate JWT, take user id from claims (needs `JWT_SECRET` + `shared` dep)
- [ ] `submit_answer`: add `token`; rename `timeToAnswer` → `timeToAnswerSeconds`; `answerId` string → int
- [ ] Drop `"q_…"`/`"a_…"`/`"sess_…"` fabricated IDs; pass question UUID through, full session UUID, integer answer index
- [ ] `QuizQuestion.id`: `Option<i32>` + hash fallback → required `Uuid`; delete fallback
- [ ] Forward `Authorization: Bearer <user token>` on both outbound calls; drop `userId` from the post-answer payload
- [ ] Log non-2xx responses from quiz/scoreboard with status + body

### shared
- [ ] Delete orphaned `src/models.rs` (broken `User` model, not in `lib.rs`)
- [ ] Remove placeholder `add()` from `lib.rs`

### docker-compose
- [ ] `SCOREBOARD_SERVICE_URL` → `http://scoreboard-service:3000` (5000 is the host mapping, unreachable in-network)
- [ ] Add `JWT_SECRET` to quiz-service and singleplayer-service
