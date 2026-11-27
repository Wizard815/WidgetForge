// Sunrise, sunset and the moon's phase, worked out on this computer (no service).
// Sun times use the standard solar-position formulas published by Astronomy
// Answers (the same ones SunCalc uses); accurate to about a minute.
// The moon phase uses the mean length of a lunar month: within about a day.

const rad = Math.PI / 180;
const DAY = 86400000;
const J1970 = 2440588;
const J2000 = 2451545;
const TILT = rad * 23.4397;

const toJulian = (ms) => ms / DAY - 0.5 + J1970;
const fromJulian = (j) => (j + 0.5 - J1970) * DAY;
const toDays = (ms) => toJulian(ms) - J2000;

const meanAnomaly = (d) => rad * (357.5291 + 0.98560028 * d);
const eclipticLongitude = (M) =>
  M + rad * (1.9148 * Math.sin(M) + 0.02 * Math.sin(2 * M) + 0.0003 * Math.sin(3 * M)) + rad * 102.9372 + Math.PI;
const declination = (L) => Math.asin(Math.sin(TILT) * Math.sin(L));

const J0 = 0.0009;
const cycle = (d, lw) => Math.round(d - J0 - lw / (2 * Math.PI));
const approxTransit = (ht, lw, n) => J0 + (ht + lw) / (2 * Math.PI) + n;
const transitJ = (ds, M, L) => J2000 + ds + 0.0053 * Math.sin(M) - 0.0069 * Math.sin(2 * L);
const hourAngle = (h, phi, dec) => Math.acos((Math.sin(h) - Math.sin(phi) * Math.sin(dec)) / (Math.cos(phi) * Math.cos(dec)));

/** Times (ms) for one day at a place. Missing values (null) during polar day or night. */
export function sunTimes(date, lat, lon) {
  const lw = rad * -lon;
  const phi = rad * lat;
  // Use local noon so "today" means the local day.
  const noonLocal = new Date(date.getFullYear(), date.getMonth(), date.getDate(), 12).getTime();
  const d = toDays(noonLocal);
  const n = cycle(d, lw);
  const ds = approxTransit(0, lw, n);
  const M = meanAnomaly(ds);
  const L = eclipticLongitude(M);
  const dec = declination(L);
  const noon = transitJ(ds, M, L);
  const at = (angle) => {
    const w = hourAngle(angle * rad, phi, dec);
    if (isNaN(w)) return [null, null];
    const set = transitJ(approxTransit(w, lw, n), M, L);
    return [fromJulian(noon - (set - noon)), fromJulian(set)];
  };
  const [sunrise, sunset] = at(-0.833);
  const [dawn, dusk] = at(-6);
  const [goldenEnd, goldenStart] = at(6);
  // Is the sun up all day or down all day?
  const middayAltitude = 90 - Math.abs(lat - dec / rad);
  return {
    sunrise, sunset, dawn, dusk, golden_morning_end: goldenEnd, golden_evening: goldenStart,
    noon: fromJulian(noon),
    day_length: sunrise && sunset ? (sunset - sunrise) / 1000 : middayAltitude > 0 ? 86400 : 0,
    polar: sunrise ? null : middayAltitude > 0 ? "day" : "night",
  };
}

const MONTH = 29.530588853;
const NEW_MOON = 2451550.1; // 6 Jan 2000
const PHASES = [
  ["New moon", "🌑"], ["Waxing crescent", "🌒"], ["First quarter", "🌓"], ["Waxing gibbous", "🌔"],
  ["Full moon", "🌕"], ["Waning gibbous", "🌖"], ["Last quarter", "🌗"], ["Waning crescent", "🌘"],
];

export function moonPhase(ms) {
  const age = (((toJulian(ms) - NEW_MOON) % MONTH) + MONTH) % MONTH;
  const fraction = age / MONTH;
  const [name, symbol] = PHASES[Math.round(fraction * 8) % 8];
  const toFull = age < MONTH / 2 ? MONTH / 2 - age : 1.5 * MONTH - age;
  return {
    age,
    name,
    symbol,
    illumination: ((1 - Math.cos(2 * Math.PI * fraction)) / 2) * 100,
    next_full: ms + toFull * DAY,
    next_new: ms + (MONTH - age) * DAY,
  };
}

export function sunAndMoon(date, lat, lon) {
  const today = sunTimes(date, lat, lon);
  const tomorrow = sunTimes(new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1), lat, lon);
  const now = date.getTime();
  const up = today.sunrise && today.sunset ? now >= today.sunrise && now < today.sunset : today.polar === "day";
  return {
    ...today,
    tomorrow_sunrise: tomorrow.sunrise,
    day_change: today.day_length - sunTimes(new Date(date.getFullYear(), date.getMonth(), date.getDate() - 1), lat, lon).day_length,
    sun_up: up,
    next: up ? today.sunset : now < (today.sunrise ?? 0) ? today.sunrise : tomorrow.sunrise,
    moon: moonPhase(now),
  };
}
