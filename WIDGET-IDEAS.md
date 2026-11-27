# Widget ideas backlog

From a look at what KDE Plasma, Android, iPhone/macOS and Rainmeter users are offered
and recommended (2026-11-26). Sources were reviewer roundups and forum activity, not
usage data. Tick items off as they are built; keep the reason with each one.

Built from this list (2026-11-26, all 30 now): World clock, Photo frame, Countdown, Month calendar,
Sports scores, Network graph + latency, Volume mixer, Disk details, Docker, Command output.

Second pass built every item below (see plans.md, seventh pass), with these changes:
Commute uses a MOTIS server you set (transitous.org forbids commercial use); Speed test uses
your own LibreSpeed server; Game servers became Minecraft Server + Steam Friends; Smart-home
buttons became Hue Lights (Home Assistant has its own widget); Email uses IMAP, not OAuth.

## Normal users

- [x] Package tracking — needs a carrier-neutral tracking service (most need a paid key).
- [x] Stock and crypto prices — display-only watchlist; check the data source's terms.
- [x] Habit tracker / streaks — local storage only, easy.
- [x] Device batteries — phone, headphones, controller (Bluetooth battery levels; KDE Connect).
- [x] Screen time today — which apps were in front, for how long (needs active-window tracking).
- [x] Internet radio — stream player (Radio Browser API is free).
- [x] Translator — needs a translation service (LibreTranslate can be self-hosted).
- [x] Commute — traffic or next transit departures (GTFS / city APIs).
- [x] Email unread count — Gmail / Outlook sign-in (OAuth, like Discord).
- [x] Sun and moon — sunrise, sunset, moon phase (can be calculated locally, no service).
- [x] Picture / quote of the day — e.g. NASA Astronomy Picture of the Day.
- [x] Currency and unit converter — currency needs a rates source.
- [x] Smart-home buttons — Hue / Home Assistant on/off (shares work with the HA panel below).
- [x] Break reminders — water, stretch, 20-20-20 eyes (local timers + a notification).
- [x] Folder portal / app launcher — a folder or apps as icons (the `folder` source exists now).

## Power users

- [x] Action buttons — Stream Deck-style grid that runs commands (reuse the command runner).
- [x] Services grid — many homelab services as up/down tiles in one widget.
- [x] Remote machine stats — other servers' CPU/RAM/disk through Glances' API.
- [x] Home Assistant panel — toggles and sensor values with buttons.
- [x] Plex / Jellyfin now streaming — who is watching what, with progress.
- [x] Download queues — qBittorrent, Sonarr, Radarr.
- [x] Pi-hole / AdGuard — stats plus "pause blocking for 5 minutes".
- [x] GitHub — notifications, pull requests, CI status.
- [x] Fans, power draw, all sensors — watts, fan speeds, every temperature.
- [x] Public IP and VPN status — including WireGuard.
- [x] Wake-on-LAN buttons.
- [x] Speed test history — scheduled speed test with a graph.
- [x] Log viewer — tail a log file or the system journal.
- [x] Game server status — Minecraft player counts, Steam friends online.
- [x] Jira / Linear / Todoist tasks — synced task lists.

## Bigger than a widget

- [x] Visual widget builder (what KWGT and Rainmeter users value most). Built: Hub → Builder.
- [x] Theming across all widgets (Plasma's "Panel Colorizer" draws a lot of interest). Built: Hub → Theme.
