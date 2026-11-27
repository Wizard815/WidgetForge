# Credits

Everything WidgetForge uses that someone else made. Keep this current.
The app shows the same on Hub → Credits.

## Data services

| Service | Used for | License / terms | Credit owed |
|---|---|---|---|
| [Open-Meteo](https://open-meteo.com/) | Weather widget: forecasts and place search | Data CC BY 4.0; free API is non-commercial | Yes: "Weather data by Open-Meteo.com" is shown in the widget and on the Credits page |

| [TheSportsDB](https://www.thesportsdb.com/) | Team Scores widget: results and fixtures | Their API terms; "123" is a free test key | "Data by TheSportsDB.com" is shown in the widget and on the Credits page |
| [Frankfurter](https://frankfurter.dev/) | Converter widget: currency rates | Free, no key; commercial use allowed, credit appreciated | "Rates from Frankfurter" in the widget |
| [Radio Browser](https://www.radio-browser.info/) | Internet Radio widget: station directory | Free and open; allowed in paid software; asks for a descriptive User-Agent and a click count per play (done) | "Stations from radio-browser.info" in the widget |
| [NASA APOD](https://api.nasa.gov/) | Picture of the Day widget | Free; DEMO_KEY is 30 calls/hour, 50/day per IP; pictures may carry their own copyright (shown) | Credit line shown with each picture |
| [Wikipedia featured feed](https://en.wikipedia.org/api/rest_v1/) | Picture of the Day widget | Wikimedia API rules: User-Agent should name the app **and give contact info** (not done yet) | Artist and license shown with each picture |
| [CoinGecko](https://www.coingecko.com/en/api) | Prices widget: crypto | Demo key officially required (works keyless today); **must show "Powered by CoinGecko"** (shown) | Yes, in the widget |
| [Finnhub](https://finnhub.io/) | Prices widget: stocks | **Free plan is personal use only**; each user brings their own key | — |
| [Ship24](https://www.ship24.com/) | Parcels widget | User's own key; free plan 10 parcels/month; commercial terms not checked | "Tracking by Ship24" in the widget |
| [Steam Web API](https://steamcommunity.com/dev) | Steam Friends widget | User's own key (never ship one); no implied Valve endorsement; needs a privacy policy | "Data from Steam. Not affiliated with Valve." in the widget |
| [ipify](https://www.ipify.org/), [icanhazip](https://icanhazip.com/) | IP & VPN widget: public IP | Terms for paid apps not confirmed | — |
| [Open-Meteo geocoding](https://open-meteo.com/) | Sun & Moon widget: city lookup | Same as Weather above | Same |

**Check before selling on Steam:** Open-Meteo's free API is for non-commercial use. A paid
app may need their commercial plan, or each user's own key. Decide before release.

TheSportsDB: check their terms for use in a paid app, and whether users should bring their own
supporter key.

Sun and moon times use the standard formulas published by Astronomy Answers (the same
ones the BSD-licensed SunCalc library uses); written fresh for WidgetForge, no code copied.

Services the user runs or signs in to themselves are reached only with the user's own address
and keys, and nothing from them ships with the app: Home Assistant, Philips Hue, Pi-hole,
AdGuard Home, Plex, Jellyfin, qBittorrent, Sonarr, Radarr, Glances, GitHub, Todoist, Linear,
Jira, LibreTranslate, LibreSpeed, MOTIS, IMAP mail, Docker, Minecraft servers.

**Not used on purpose:** transitous.org (its rules forbid commercial use and require open
source), Cloudflare's speed-test endpoints (no published terms), ip-api.com (non-commercial).

User-added feeds, calendars and services (RSS, ICS, Live App, LLM providers) are reached
with the user's own links and keys; nothing from them ships with the app.

## Art

| Item | Source | Status |
|---|---|---|
| App icon | Code-drawn stand-in (`tools/placeholder_icon.py`) | Replace with the owner's icon |
| Widget icons (`widgets/*/icon.svg`) | Unknown, came with the plans | Owner to confirm source and AI status |

No sounds, music or fonts are bundled. Widgets use the system font.

## Software

438 third-party packages ship in the Linux build (Rust crates compiled into the app, plus
Svelte and @tauri-apps/api in the page). All are permissive: MIT, Apache-2.0, ISC, BSD,
Zlib, Unicode-3.0, Unlicense, CC0, plus 5 MPL-2.0 crates (fine to ship unmodified; if one is
changed, that file's source must be shared).

- Full list: `public/credits/third-party.json` (shown in the Hub).
- Full license texts: `public/credits/THIRD-PARTY-NOTICES.txt` (ships with the app; MIT,
  Apache and BSD require this).
- Regenerate after any dependency change, on each platform you ship (Windows pulls in
  different crates): `python3 tools/gen_credits.py`.
- 7 packages have no license file inside them (alloc-stdlib, dlopen2, dlopen2_derive, enum-kinds, libappindicator-sys, selectors, tauri-plugin). Their license is named in the
  list; add the standard license text for them before release.

Main pieces: Tauri (MIT/Apache-2.0), Svelte (MIT), sysinfo (MIT), reqwest (MIT/Apache-2.0),
serde (MIT/Apache-2.0), arboard (MIT/Apache-2.0), mpris (Apache-2.0),
starship-battery (ISC), keyring (MIT/Apache-2.0). Build tools not shipped: Vite (MIT), Rust and Node toolchains.
