# Kolom Khela: developer guide

How the whole system works, from the browser to the database, including the live deployment. Read this before
changing anything, and use the FAQ at the end when someone asks "why is it like this?".

The README covers what the game is and how to run it. This file covers how it works.

---

## 1. The whole system on one page

```
 Browser (static page, Bangla UI)
   │  HTTPS  /api/...      JSON, Bearer token     → accounts, shop, profile, admin, create room
   │  WSS    /ws/:code     JSON messages          → everything that happens inside a match
   ▼
 One Rust process (axum + tokio)            ← also serves the built page files from web/out
   ├─ api.rs     HTTP routes + the WebSocket handler
   ├─ room.rs    one tokio task per live room: seats, turns, rules, results   (in memory only)
   ├─ game.rs    physics: pens as capsules, collisions, friction, falling off the desk
   ├─ bot.rs     the computer player
   └─ db.rs      SQLite file locally, Postgres (Neon) in production, same code for both
```

Three ideas explain almost every design choice:

1. **The server is the only authority.** The browser never moves a pen. It sends "I flick with this direction,
   power and finger position" and the server runs the physics and tells everyone what happened. A modified
   browser can only send a different flick on its own turn, which is something a player could do anyway.
2. **One process, one thing to host.** The Next.js app is exported to plain HTML/JS/CSS (`output: 'export'`)
   and served by the Rust binary. There is no Node server in production, no CORS, no second service.
3. **Live state lives in memory, durable state in the database.** Rooms, turns and pen positions exist only
   inside the running process. Accounts, coins, pens, pictures and finished games are in the database. A restart
   or deploy ends every match in progress but loses no account.

---

## 2. Repository map

```
server/                     Rust, the backend and the game
  src/main.rs               start-up: env vars, DB connect + migrate, admin seed, router, static files
  src/api.rs                every HTTP route, and play() which runs one WebSocket connection
  src/room.rs               Room task, Difficulty, seats, turn order, lives, results, the computer's turn
  src/game.rs               physics (pure, no I/O) + tests
  src/bot.rs                the computer player (pure, no I/O) + tests
  src/pens.rs               the pen catalogue (size, weight, price) and reward constants
  src/db.rs                 connection, migrations, reward_win, save_game, the `?` → `$1` query rewrite
  src/auth.rs               password hashing, session tokens, user lookup
  src/guard.rs              rate limiter, password rules, security headers, session lifetime
  src/email.rs              the email address filter (typos, test domains, throwaway services)
web/                        Next.js 16, exported to static files in web/out
  app/page.js               home: sign in / sign up, lobby (new game, vs computer, join by code), leaderboard
  app/room/page.js          reads ?code= (and ?bot=1) and mounts Game
  app/room/Game.js          the match screen: WebSocket, canvas loop, aiming, lobby panel, prompts
  app/room/draw.js          everything drawn on the canvas: desks, pens, aim preview, turn hint
  app/room/panels.js        level picker, player cards, lives, end-of-game overlay, confetti, coin counter
  app/shop/ app/profile/ app/admin/   the other pages
  lib/api.js                fetch wrapper + token storage + WebSocket URL
  lib/pens.js               how each pen looks (colours, cap, rings); the physics sizes come from the server
  lib/sound.js              synthesized sound effects (Web Audio, no files)
Dockerfile                  builds web, builds server, ships both in one small image
```

---

## 3. The live system

| Piece | What | Notes |
|---|---|---|
| App | Render web service, Docker, free tier | Public URL defaults to `https://kolom-khela.onrender.com` (see `web/app/layout.js`) |
| Database | Neon Postgres, free tier | Set as `DATABASE_URL`. Use the **direct** connection string, not the pooled one |
| Code | GitHub `zkrana/kolom-khela`, branch `main` | Render builds from it with the `Dockerfile` |

**Environment variables** (set in Render → Environment):

| Name | Purpose |
|---|---|
| `DATABASE_URL` | Postgres URL. Without it the server uses a SQLite file, which a free host wipes on every restart |
| `ADMIN_EMAIL`, `ADMIN_PASSWORD` | On every start the account with this email is created, or promoted to admin and unblocked |
| `PORT` | Set by Render |
| `NEXT_PUBLIC_SITE_URL` | Docker **build** argument, so WhatsApp/Facebook link previews point at the real domain |
| `STATIC_DIR`, `DB_PATH` | Already set inside the Docker image |
| `DEV_CORS` | Local development only |

**What players notice about the free tier**

- The service sleeps after about 15 minutes with no traffic and takes about a minute to wake on the next visit.
- Any restart or deploy ends the matches in progress. Accounts, coins and pens are safe in Neon.
- After a deploy, a browser can show the old page for up to 5 minutes: static files are sent with
  `Cache-Control: public, max-age=300`. API responses are `no-store`. A hard refresh gets the new page at once.
- Logs are whatever the process prints (start-up lines, DB errors), visible in Render → Logs.

**Capacity.** One process holds at most 200 live rooms (`create_room` refuses beyond that), 4 seats each. A room
costs one tokio task stepping physics at 60 Hz only while pens are moving. The real limits on the free tier are
CPU and the 15-minute sleep, not memory.

---

## 4. Life of a match

1. **Create.** `POST /api/rooms` (signed in) makes a 5-letter code (no 0/O/1/I) and spawns a room task. The
   creator is the host.
2. **Connect.** The page opens `wss://…/ws/CODE?token=…`. `api::play` checks the token, sends `Join` to the
   room, then forwards messages both ways. Every connection gets the full `state` immediately.
3. **Lobby.** Up to 4 seats. The host picks the level and can add computer players. Anyone arriving to a full
   lobby takes a computer's chair if there is one, otherwise they watch. The desk already shows each pen at its
   start spot.
4. **Start.** Host only, at least 2 seats (people or computers). Seats whose person disconnected are dropped.
5. **A turn.** The player on turn presses and pulls back on the canvas. On release the page sends `shoot`. The
   room checks it is that player's turn, applies `game::flick`, and steps physics every 16 ms. Every second step
   (about 30 per second) it broadcasts a `frame` with pen positions. The page smooths between frames.
6. **The shot settles.** When nothing moves, `finish_turn` handles every pen that fell: one life lost, a knock
   credited to the shooter (not for knocking yourself off), and either a respawn on its own edge or, with no
   lives left, out of the game. Then the turn moves on, round the desk.
7. **The end.** One player left, or the turn limit reached. `finish_game` ranks everyone, pays the winner,
   saves the game, and sends `phase: "over"` with the results. The page shows the end screen.
8. **Rematch.** The host sends `rematch`: back to the lobby with everyone still at the desk, plus anyone who was
   watching (they take the free chairs).
9. **Cleanup.** A room with no person connected for 60 seconds removes itself. Computers do not keep it alive.

---

## 5. WebSocket protocol

All messages are JSON with a `t` field. Messages over 512 bytes are ignored; more than 60 messages in 10 seconds
closes the connection.

**Browser → server**

| `t` | Fields | Who | Effect |
|---|---|---|---|
| `start` | | host, lobby | start the match |
| `difficulty` | `level`: `easy` / `medium` / `hard` | host, lobby | change the level |
| `bot_add` | | host, lobby, < 4 seats | seat a computer player |
| `bot_remove` | `id` (the computer's negative id) | host, lobby | remove it |
| `shoot` | `dx, dy` direction, `power` 0–1, `lx, ly` finger point on the pen in its own frame | player on turn | flick |
| `rematch` | | host, after the end | back to the lobby |

Anything not allowed (wrong person, wrong phase, not your turn) is silently ignored.

**Server → browser**

| `t` | Meaning |
|---|---|
| `state` | Full snapshot: `phase`, `host`, `difficulty`, `players[]` (seat, id, name, pen, avatar, alive, online, bot, lives, knocks, falls), `turn`, `order` (turn order of seats), `simulating`, `left_ms` (turn clock), `bodies[]` (pens on the desk), `results` (after the end), `reward`, `maxLives`, `turnSecs`, and `shot` (the physics numbers the aim preview needs) |
| `frame` | During a shot: `b: [[id, x, y, angle], …]` and `h` (hardest hit, for the click sound) |
| `respawn` | `ids` of pens that came back this turn, for the flash and sound |
| `aim` | A computer is about to flick: same fields as `shoot` plus `seat`; the page draws it like a finger |
| `log` | One line for the events panel (Bangla) |
| `error` | Something the player should read, e.g. "need 2 players" |
| `closed` | The room was closed by an admin |

A pen's `id` on the desk is its owner's **seat index**, so `players[id]` is the owner.

---

## 6. Rules and numbers

**Levels** (`room::Difficulty`):

| | Easy | Medium (default) | Hard |
|---|---|---|---|
| Lives | 3 | 2 | 1 |
| Flick speed (share of `MAX_SPEED` 1500) | 60% | 72% | 85% |
| Turn clock | 35 s | 25 s | 18 s |
| Turn limit | 80 | 60 | 40 |
| Winner's coins | 5 | 10 | 20 |

- **Seats and start spots.** Seat 0 bottom, 1 top, 2 left, 3 right, each 175 px in from its edge on an 800×800
  desk (`game::home_spot`). Three players use bottom, top, left.
- **Turn order** goes round the desk, clockwise on screen: 2 players 0→1, 3 players 0→2→1, 4 players
  0→2→1→3 (`room::ring`). It is not join order.
- **Falling.** A pen whose middle leaves the desk has fallen. It costs one life. With lives left it comes back
  at its own start spot, sliding along the edge if another pen is there (`game::respawn`).
- **Out.** No lives left. The first player out is the **looser** (the spelling is the game's own slang, also
  used in field names). If several go out on one shot, the shooter's own pen counts as falling first, so knocking
  yourself off never earns a better place than the pens you took with you.
- **Clock.** A turn that runs out is skipped. Three skipped turns in a row and the player is out
  (`MAX_MISSED`), so nobody waits out a match for someone who walked away. A player whose connection dropped
  gets 3-second turns (`AWAY_SECS`), and so goes out within about 10 seconds unless they come back.
- **Turn limit reached.** Survivors are ranked by lives left, then by how close their pen is to the middle.
- **Host leaves** (any phase). The next person at the desk becomes host. Computers are never host.
- **Coins.** Only the winner is paid, through `db::reward_win`. At most 15 paid wins per player per 24 hours
  (`pens::DAILY_REWARDED_WINS`), so two accounts cannot farm each other. If the only opponents were
  computers, the winner gets half. A computer that wins is paid nothing (it has no account).

---

## 7. Physics (`game.rs`)

- Pens are **capsules**: a straight segment (`hl` = half length) with round ends (`r`). Mass `m` comes from the
  pen catalogue; rotational inertia is that of a rod.
- A **flick** sets the pen's velocity along the flick direction, and its spin from where the finger was
  (`lx, ly`): press the middle and it slides straight, press near an end and it spins too.
- Each step (1/60 s): move, apply exponential friction (`DAMP` 2.8 per second on speed, `SPIN_DAMP` 3.5 on
  spin), stop anything slower than 7 px/s, then resolve every touching pair: closest points between the two
  segments (Ericson, Real-Time Collision Detection 5.1.9), push them apart by mass, and apply an elastic impulse
  (restitution 0.88) that includes spin.
- **The useful identity:** with no collisions a pen slides `speed / DAMP` pixels. The aim preview uses exactly
  this, with the constants the server sends in `state.shot`, so the dotted path and the ghost pen show where the
  pen will really stop. The 175 px start margin is derived from it too: a half-power flick straight back at your
  own edge is safe on Easy and fatal on Hard. Tests in `game.rs` hold both facts; if you change `DAMP`, the
  speeds or the margin, those tests tell you what you broke.

---

## 8. The computer player (`bot.rs`)

- It only ever produces a flick, which the room applies through the same `Room::shoot` as a person's. It has no
  other way to touch the game.
- **How it chooses.** For every other pen it tries aiming at the middle and towards both ends, at five powers,
  with and without spin, plus a few gentle slides towards the centre: about 30 to 100 flicks. Each one is played
  out on a copy of the desk with the real physics. Score: knocking a pen off is good (much better if it is that
  player's last life), losing its own pen is very bad (and never acceptable on its last life), and where things
  come to rest counts too (its own pen away from the edges, others near them).
- **Levels.** Easy picks among its 6 best shots and its aim wanders ±0.13 rad and power ±20%. Medium: best 3,
  ±0.06 rad, ±10%. Hard: always the best, ±0.02 rad, ±4%. Without this shake it would be near perfect, because it
  sees the same physics the server runs.
- **Timing.** It "thinks" for 0.7–1.4 s, broadcasts its `aim` (drawn like a person's pull-back), and flicks
  0.9 s later. Its turn has no clock on screen.
- **Identity.** Names `রোবো রাজু`, `রোবো মিতু`, `রোবো টুনি`, always the Econo pen. Negative `user_id`s; saved
  in `game_players` (so admin sees who won), filtered out of the leaderboard.
- Cost: the whole choice takes a few milliseconds; a test plays four computers through complete Easy and Hard
  games in under 0.1 s.

---

## 9. Data model

Tables are created on start-up by `db::migrate` (no migration tool; new columns are added with `ALTER TABLE …`
that is allowed to fail if the column exists). The same SQL runs on SQLite and Postgres.

| Table | Columns | Notes |
|---|---|---|
| `users` | id, email (unique), name, pass_hash, role (`player`/`admin`), blocked, created_at, coins, pen (selected), pens (owned, comma list), guest, contact | Guests have a placeholder email `guest-…@guest.invalid` and their real email in `contact` |
| `sessions` | token_hash (SHA-256 of the token), user_id, created_at | The token itself is never stored. 30-day life |
| `avatars` | user_id, mime, data (blob), updated_at | One picture per user, ≤ 300 KB, JPEG/PNG/WebP checked by magic bytes |
| `games` | id, code, finished_at, winner_id, loser_id | One row per finished match |
| `game_players` | game_id, user_id, name, coins, rank, looser | Stats, leaderboard and the daily reward cap are computed from these |

Times are Unix seconds. Queries are written with `?`; `db::q` rewrites them to `$1, $2…` on Postgres once and
caches the result.

---

## 10. Accounts and security

**Accounts**

- Sign-up: email, name (2–24 characters), password (6–100, and not one of the 20 most common).
- Passwords are hashed with Argon2. Sessions are 32 random bytes; the browser keeps the token in
  `localStorage` (`kolom_token`) and sends it as `Authorization: Bearer …`, or as `?token=` on the WebSocket.
- **Guest join:** an invite link asks only for name and email. That creates a password-less guest account. The
  profile page lets a guest "save" it with a real email and password, keeping coins and pens.
- **Blocked** users fail every token check, so blocking also ends their live sessions.
- The admin is whoever matches `ADMIN_EMAIL` at start-up, plus anyone an admin promotes.

**Email filter** (`email.rs`), used by sign-up, guest join and guest upgrade: checks the shape, then refuses
common typos and names the fix ("did you mean …@gmail.com?" for `gmaill.com`, `gmial.com`, `yahooo.com` …),
placeholder domains (`test.com`, `example.com` …), throwaway services (`mailinator.com`, `yopmail.com` …) and our
own guest domain.

**Limits** (`guard::Limiter`, in memory, per process):

| What | Limit |
|---|---|
| Sign-ups | 30 per hour per IP (loose on purpose: a classroom shares one IP) |
| Guest joins | 40 per 10 minutes per IP |
| Login | 9 tries per 5 minutes per email, 31 per IP; a correct password clears the email's count |
| New rooms | 12 per 10 minutes per user, 200 rooms in total |
| Picture uploads | 20 per 10 minutes per user |
| WebSocket | 60 messages per 10 s, 512 bytes each |
| Request body | 64 KB (400 KB on the picture upload route) |

**Headers on every response:** a Content Security Policy (only our own origin; `unsafe-inline` for scripts is
required by the Next.js static export), `X-Frame-Options: DENY`, `nosniff`, `Referrer-Policy: no-referrer`, and a
Permissions-Policy that turns off camera, microphone, location, payment and USB.

**What a cheater can and cannot do.** They can send any flick on their own turn (the server clamps power and
checks numbers are finite). They cannot move pens, act on someone else's turn, change the level mid-match, give
themselves coins, or buy without paying (the purchase is one conditional `UPDATE`).

---

## 11. Frontend notes

- All interface text is Bangla, `<html lang="bn">`.
- `Game.js` keeps live data in refs (positions, aim) and draws with `requestAnimationFrame`; React state is only
  for what the side panels show. Pen positions move 55% of the way to the latest server position every frame,
  which smooths 30 Hz updates to the screen's rate.
- The desk is painted once per theme into an off-screen canvas and copied each frame. Themes: bench, slate,
  felt, ocean, notebook page. Each pen has a halo in its owner's seat colour, so two players with the same pen
  model are still told apart.
- Per-browser settings: `kolom_desk` (theme), `kolom_mute` (sound) in `localStorage`; `kolom_next` in
  `sessionStorage` brings a guest back to the room after signing in.
- The pen's **look** is in `web/lib/pens.js`; its **size and weight** come from the server in `state.bodies`.

---

## 12. Running and testing

```bash
cd web && npm install && npm run build && cd ..
cd server
ADMIN_EMAIL=you@example.com ADMIN_PASSWORD='a-long-password' cargo run --release   # http://localhost:8080
cargo test                                                                          # 22 tests
```

For page work with hot reload: run the server with `DEV_CORS=1` and `cd web && npm run dev` (port 3000, talks to
8080 through `NEXT_PUBLIC_API`).

`cargo test` covers the physics (falls, spin, respawn, the Easy/Hard margin), the computer (takes an easy knock,
never drops its own last life, four computers finish whole games), the turn order, the email filter, the rate
limiter and the password rule. The API and multiplayer behaviour (lobby, host passing, walk-away rule, tab
handling, computers, rewards) has been checked with scripts against a running server; those scripts are not
in the repository yet.

---

## 13. Common changes

| To | Change | Watch out for |
|---|---|---|
| Add a pen | `server/src/pens.rs` (size, weight, price) and its look in `web/lib/pens.js` with the same `id` | Heavier and longer pens change the game balance |
| Change coins per win | `Difficulty::reward` in `room.rs` | The home-page rules text in `web/app/page.js` quotes the numbers |
| Change level numbers | `Difficulty` in `room.rs`, and `LEVELS` in `web/app/room/panels.js` (labels and blurbs) | Run `cargo test`; the margin tests guard speed vs start spot |
| Change the daily cap | `DAILY_REWARDED_WINS` in `pens.rs` | |
| Make the computer stronger/weaker | the `(choosy, angle, power)` table in `bot::choose` | |
| Add a desk theme | `DESKS` and `paintDesk` in `web/app/room/draw.js` | |
| Add a WebSocket command | `ClientMsg` + the match in `api::play`, a `Cmd` variant and its handler in `room.rs` | Check host/phase/turn in the handler: the server must refuse what is not allowed |
| Add a database column | append an `ALTER TABLE … ADD COLUMN …` to the list in `db::migrate` | Must work on SQLite and Postgres |

---

## 14. FAQ

**Why Rust and not Node?** Small, fast, one binary that also serves the page. The physics runs on the server for
every room, and a free instance handles many rooms on very little CPU and memory.

**Why WebSocket and not polling?** Positions stream about 30 times a second during a shot. Plain WebSocket,
no Socket.io: the browser API is enough and there is nothing extra to load.

**Why does the server run the physics instead of the browser?** Fairness and agreement. Every screen shows the
same result, and nobody can cheat by editing the page.

**Why is the page a static export?** No Node server to host, so the page loads as plain files from the same
place as the API, with no cold start of its own and no cross-site setup.

**What happens when the server restarts?** Matches in progress end and players see "reconnecting", then "room
closed". Accounts, coins, pens, pictures and finished games are kept (Postgres).

**Can it run on two servers?** Not as it is. Rooms live in one process's memory, so every player of a room must
reach the same process. Scaling out would need routing by room code (sticky routing) or a shared room store.

**Why both SQLite and Postgres?** SQLite for running locally with nothing to install, Postgres in production
because the free host has no disk that survives a restart. `sqlx`'s `Any` driver runs the same code on both.

**Does the computer cheat?** It sees the desk, as every player does, and tries flicks on a copy using the real
physics, which a person cannot. That is why it deliberately shakes its aim and power, more on easier levels.

**Why does a fallen pen come back?** With one life per player the first slip ended the game; players found that
too abrupt. Lives and respawn make a match last several rounds. Hard keeps the one-life rule.

**Why "looser"?** That is how players say it. It stays in field names (`looser`) so the code and the game use one
word.

**Where is the turn order decided?** `room::ring`. It goes round the desk, not by who joined first.

**What if someone opens the game in two tabs?** The newer connection takes the seat. Closing either tab does not
unseat the player while the other is still open.

---

## 15. Known limits

- Single instance only (see FAQ). Rooms are lost on restart or deploy.
- Free-tier sleep: the first visit after about 15 quiet minutes waits about a minute.
- IP-based limits trust the first `X-Forwarded-For` entry, which a client can set. They slow down a script; they
  are not a wall. The per-email login limit does not depend on it.
- The session token travels in the WebSocket URL, so it can appear in proxy access logs. It is a 30-day
  game session, not a payment credential.
- The CSP must allow inline scripts because of the static export.
- The daily reward cap counts every paid win, including wins against the computer.
- No email verification: the filter stops typos and fake domains, not someone typing a real address that is not
  theirs.
