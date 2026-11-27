// A small, safe calculator: numbers, + - * / % ^, parentheses, unary minus.
// It never runs code; it parses the expression itself.

export function calculate(input) {
  const src = String(input).replace(/×/g, "*").replace(/÷/g, "/").replace(/\s+/g, "");
  let i = 0;

  const peek = () => src[i];
  const take = (ch) => (src[i] === ch ? (i++, true) : false);

  function number() {
    const m = src.slice(i).match(/^(\d+\.?\d*|\.\d+)(e[+-]?\d+)?/i);
    if (!m) throw new Error("Expected a number");
    i += m[0].length;
    return parseFloat(m[0]);
  }
  function primary() {
    if (take("(")) {
      const v = expr();
      if (!take(")")) throw new Error("Missing )");
      return v;
    }
    if (take("-")) return -primary();
    if (take("+")) return primary();
    return number();
  }
  function power() {
    const base = primary();
    return take("^") ? Math.pow(base, unaryPower()) : base;
  }
  function unaryPower() {
    if (take("-")) return -unaryPower();
    return power();
  }
  function term() {
    let v = power();
    for (;;) {
      if (take("*")) v *= power();
      else if (take("/")) v /= power();
      else if (take("%")) v %= power();
      else return v;
    }
  }
  function expr() {
    let v = term();
    for (;;) {
      if (take("+")) v += term();
      else if (take("-")) v -= term();
      else return v;
    }
  }

  if (!src) return 0;
  const v = expr();
  if (i < src.length) throw new Error(`Unexpected "${peek()}"`);
  if (!Number.isFinite(v)) throw new Error("Not a number");
  // Hide float noise like 0.1 + 0.2 = 0.30000000000000004.
  return Number(v.toPrecision(12));
}
