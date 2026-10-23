# Kolom Khela

The childhood pen-flicking game from school benches, live in the browser. 2 to 4 players share one desk, one kolom (pen) each. On your turn you put your finger on your pen and flick it: touch the middle and it slides, touch near an end and it spins. Hit the other pens. A pen that leaves the desk is out; the first one out is the **looser**, the last one on the desk wins **10 coins**. Spend coins in the pen shop on heavier or longer pens (Econo, Matador, Gel grip, Fat marker, Steel fountain pen).

- **Server:** Rust (axum, WebSocket, SQLite). It owns all the physics and the rules, so nobody can cheat from the browser.
- **Web:** Next.js, exported as plain static files and served by the same Rust process. No Node server in production.
- **Accounts:** email, name and password only. Players invite friends with a room link or a 5-letter code.
- **Profile:** `/profile/` lets a player add a picture (cropped and shrunk in the browser, stored in the database), rename and change password.
- **Admin:** `/admin/` has an overview, a searchable player list (block, make admin, give or take coins, remove a picture, delete), live rooms (close) and recent games.
- **Desk and sound:** four desk looks (school bench, green felt, ocean, classic) and synthesized sound effects, both chosen per browser.

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

Tests for the physics: `cd server && cargo test`.

## Settings (environment variables)

| Name | Default | Meaning |
|---|---|---|
| `PORT` | 8080 | Port to listen on (most hosts set this) |
| `DB_PATH` | kolom.db | SQLite file |
| `STATIC_DIR` | ../web/out | The built page |
| `ADMIN_EMAIL`, `ADMIN_PASSWORD` | none | Creates (or promotes) the admin account at start-up |
| `DEV_CORS` | off | Set to anything for local `npm run dev` |

## Free hosting

Build with the included `Dockerfile` (one container, about 10 MB of Rust binary plus the page).

- **Render** (free web service): New > Web Service > connect the repo > Environment: Docker. Add `ADMIN_EMAIL` and `ADMIN_PASSWORD`. WebSockets work. The free service sleeps after about 15 minutes idle and takes about a minute to wake.
- **Koyeb** free instance: same Docker flow.
- Rooms live in memory, so a restart ends games in progress.

**Accounts and scores:** SQLite is a file. Free plans without a disk lose it on every redeploy or restart, which wipes accounts. For lasting accounts, either attach a small persistent disk (Fly.io volume, Render paid disk), or move the three tables to a free hosted database such as Turso or Neon. The admin account is re-created from the environment variables each start, so you can always log in.

## Rules recap

1. Touch your kolom, pull back, let go. The longer the pull, the harder the flick. Where you touch decides how it spins.
2. A kolom whose middle leaves the desk is out. The first one out is the looser.
3. Last kolom on the desk wins 10 coins (at most 15 rewarded wins per player per 24 hours, so two accounts cannot farm each other).
4. 25 seconds per turn, then the turn is skipped. After 60 flicks with nobody falling, the kolom nearest the middle wins.
