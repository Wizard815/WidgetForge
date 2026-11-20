// An HTML widget: plain JS with the injected `forge` SDK (see docs/DESIGN.md §5.4).
const rowsEl = document.getElementById("rows");

function periods(now, s) {
  const startOfDay = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const day = 86400000;
  const offset = s.week_starts_monday ? (now.getDay() + 6) % 7 : now.getDay();
  const startOfWeek = new Date(startOfDay.getTime() - offset * day);
  const startOfMonth = new Date(now.getFullYear(), now.getMonth(), 1);
  const startOfNextMonth = new Date(now.getFullYear(), now.getMonth() + 1, 1);
  const startOfYear = new Date(now.getFullYear(), 0, 1);
  const startOfNextYear = new Date(now.getFullYear() + 1, 0, 1);
  const frac = (a, b) => (now - a) / (b - a);
  const list = [
    ["Today", frac(startOfDay, new Date(startOfDay.getTime() + day))],
    ["This week", frac(startOfWeek, new Date(startOfWeek.getTime() + 7 * day))],
    ["This month", frac(startOfMonth, startOfNextMonth)],
  ];
  if (s.show_year) list.push([String(now.getFullYear()), frac(startOfYear, startOfNextYear)]);
  return list;
}

function render() {
  const s = forge.settings.current;
  rowsEl.innerHTML = "";
  for (const [name, f] of periods(new Date(), s)) {
    const pct = (f * 100).toFixed(1);
    const row = document.createElement("div");
    row.className = "row";
    row.innerHTML = `<div class="top"><span class="name"></span><span class="pct">${pct}%</span></div>
      <div class="track"><div class="fill" style="width:${pct}%"></div></div>`;
    row.querySelector(".name").textContent = name;
    rowsEl.appendChild(row);
  }
}

forge.ready.then((ctx) => {
  document.documentElement.style.setProperty("--accent", ctx.manifest.style.accent || "#a0e86f");
  render();
  setInterval(render, 30000);
});
forge.settings.onChange(render);
