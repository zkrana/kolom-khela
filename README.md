# Kolom Khela

The childhood pen-flicking game from school benches, live in the browser. 2 to 4 players share one desk, one kolom (pen) each, and empty chairs can be filled with computer players. On your turn you put your finger on your pen and flick it: touch the middle and it slides, touch near an end and it spins. Knock the other pens off the desk. A fallen pen costs a life and comes back; out of lives, you are out. The first one out is the **looser**, the last one standing wins coins. Spend coins in the pen shop on heavier or longer pens (Econo, Matador, Gel grip, Fat marker, Steel fountain pen).

**How it works inside, the live setup, and answers to common developer questions: [DEVELOPER.md](DEVELOPER.md).**

- **Server:** Rust (axum, WebSocket, SQLite). It owns all the physics and the rules, so nobody can cheat from the browser.
- **Web:** Next.js, exported as plain static files and served by the same Rust process. No Node server in production.
- **Accounts:** email, name and password only. Players invite friends with a room link or a 5-letter code.
- **Profile:** `/profile/` lets a player add a picture (cropped and shrunk in the browser, stored in the database), rename and change password.
- **Admin:** `/admin/` has an overview, a searchable player list (block, make admin, give or take coins, remove a picture, delete), live rooms (close) and recent games.
- **Levels:** easy, medium and hard, picked by the host (lives, flick speed, turn clock and coins all change).
- **Computer players:** play alone against the computer, or fill empty chairs. It aims with the real physics and misses more on easier levels.
- **Desk and sound:** five desk looks (school bench, slate, green felt, ocean, notebook page) and synthesized sound effects, both chosen per browser.

## Run it locally

```bash
# 1. build the page once
cd web && npm install && npm run build && cd ..

# 2. start the server (creates kolom.db and the admin account)
cd server
ADMIN_EMAIL=you@example.com ADMIN_PASSWORD='a-long-password' cargo run --release
# open http://localhost:8080
```

For live editing of the page: run the server with `DEV_CORS=1`, then `cd web && npm run dev` (talks to port 8080).

Tests (physics, computer player, turn order, email filter, limits): `cd server && cargo test`.

## Settings (environment variables)

| Name | Default | Meaning |
|---|---|---|
| `PORT` | 8080 | Port to listen on (most hosts set this) |
| `DATABASE_URL` | none | `postgres://...` (for example a free Neon database). Keeps players and coins when a free host restarts. Without it a local SQLite file is used |
| `DB_PATH` | kolom.db | SQLite file, only used when `DATABASE_URL` is not set |
| `STATIC_DIR` | ../web/out | The built page |
| `ADMIN_EMAIL`, `ADMIN_PASSWORD` | none | Creates (or promotes) the admin account at start-up |
| `DEV_CORS` | off | Set to anything for local `npm run dev` |

## Free hosting

Build with the included `Dockerfile` (one container, about 10 MB of Rust binary plus the page).

- **Render** (free web service): New > Web Service > connect the repo > Environment: Docker. Add `ADMIN_EMAIL` and `ADMIN_PASSWORD`. WebSockets work. The free service sleeps after about 15 minutes idle and takes about a minute to wake.
- **Koyeb** free instance: same Docker flow.
- Rooms live in memory, so a restart ends games in progress.

**Accounts and scores:** on a free host with no disk, a SQLite file is lost on every restart. Set `DATABASE_URL` to a free Neon Postgres database (use the direct, not the pooled, connection string) and everything is kept. The tables are created on first start. The admin account is created from `ADMIN_EMAIL` and `ADMIN_PASSWORD` if it does not exist yet.

## Rules recap

1. Touch your kolom, pull back, let go. The longer the pull, the harder the flick. Where you touch decides how it spins. The dotted line shows where it will stop.
2. A kolom whose middle leaves the desk loses a life and comes back on its own edge. No lives left: out. The first one out is the looser.
3. | | Easy | Medium | Hard |
   |---|---|---|---|
   | Lives | 3 | 2 | 1 |
   | Turn clock | 35 s | 25 s | 18 s |
   | Winner's coins | 5 | 10 | 20 |

   Against only computers the winner gets half. At most 15 paid wins per player per 24 hours, so two accounts cannot farm each other.
4. Turns go round the desk. A turn that runs out is skipped; three in a row and that player is out.
5. If nobody falls for a long time (80 / 60 / 40 turns), the game ends: most lives left wins, then the kolom nearest the middle.
