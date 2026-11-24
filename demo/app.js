import { init, presets } from "dealbreaker";
import { WorkerSolver } from "dealbreaker/worker";

const SUIT_SYMBOL = { C: "♣", D: "♦", H: "♥", S: "♠" };
const RED_SUITS = new Set(["D", "H"]);

const $ = (id) => document.getElementById(id);
const elPreset = $("preset");
const elSeed = $("seed");
const elMsWrap = $("ms-wrap");
const elMsMode = $("ms-mode");
const elStatus = $("status");
const elBoard = $("board");
const elLegalTitle = $("legal-title");
const elLegalList = $("legal-list");
const elRulesEditor = $("rules-editor");
const elRulesError = $("rules-error");
const elPlayback = $("playback");
const elStepCounter = $("step-counter");

const btnDeal = $("btn-deal");
const btnSolve = $("btn-solve");
const btnHint = $("btn-hint");
const btnCancel = $("btn-cancel");
const btnUndo = $("btn-undo");
const btnShare = $("btn-share");
const btnFirst = $("btn-first");
const btnPrev = $("btn-prev");
const btnPlay = $("btn-play");
const btnNext = $("btn-next");
const btnApplyRules = $("btn-apply-rules");
const btnResetRules = $("btn-reset-rules");

let worker = null;
let currentPreset = "freecell";
let isCustomRules = false;
let currentRules = null;
let state = null;
let legalSteps = [];
let history = [];
let selectedPile = null; // { group, index }
let hintedStep = null;
let busy = false;

// Solution playback state
let playbackStates = [];
let playbackSteps = [];
let playbackIdx = 0;
let playTimer = null;

function setStatus(msg) {
  elStatus.textContent = msg;
}

function setBusy(on) {
  busy = on;
  btnSolve.disabled = on;
  btnHint.disabled = on;
  btnDeal.disabled = on;
  btnApplyRules.disabled = on;
  btnCancel.disabled = !on;
  btnUndo.disabled = on || history.length === 0;
}

function stopPlayback() {
  if (playTimer !== null) {
    clearInterval(playTimer);
    playTimer = null;
    btnPlay.textContent = "▶ Play";
  }
}

function clearPlayback() {
  stopPlayback();
  playbackStates = [];
  playbackSteps = [];
  playbackIdx = 0;
  elPlayback.classList.add("hidden");
}

function isScored() {
  return Boolean(currentRules && currentRules.score);
}

function formatStep(step) {
  if (step.type === "deal") return "deal";
  if (step.type === "redeal") return "redeal";
  if (step.type === "move") {
    const cnt = step.count > 1 ? ` (${step.count})` : "";
    return `${step.from.group} ${step.from.index + 1} → ${step.to.group} ${step.to.index + 1}${cnt}`;
  }
  if (step.type === "remove") {
    return "remove " + step.piles.map((p) => `${p.group} ${p.index + 1}`).join(" + ");
  }
  return JSON.stringify(step);
}

function samePile(a, b) {
  return a && b && a.group === b.group && a.index === b.index;
}

function sameStep(a, b) {
  return a && b && JSON.stringify(a) === JSON.stringify(b);
}

const DEFAULT_SUITS = ["clubs", "diamonds", "hearts", "spades"];
const DEFAULT_RANKS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

function eqArr(a, b) {
  return Array.isArray(a) && a.length === b.length && a.every((v, i) => v === b[i]);
}

function stripDefaults(obj, defaults) {
  const out = {};
  for (const [k, v] of Object.entries(obj)) {
    if (k in defaults && v === defaults[k]) continue;
    out[k] = v;
  }
  return out;
}

function compactBuild(b) {
  return stripDefaults(b, { dir: "down", wrap: false, step: 1 });
}

function compactGroup(g) {
  const out = stripDefaults(g, {
    count: 1,
    complete_runs: false,
    compact: false,
    requires_rank_on_prev: false,
    match_longest_rank: false,
    accept_only_when_empty: false,
    accept_when_stock_empty: false,
    compact_face_up: false,
    refill_largest: false,
    refill_on_empty: false,
  });
  if (out.build) out.build = compactBuild(out.build);
  if (Array.isArray(out.piles)) {
    out.piles = out.piles.map((p) => {
      const po = { ...p };
      if (po.build) po.build = compactBuild(po.build);
      return po;
    });
  }
  return out;
}

/** Strip fields matching their exact serde defaults so the Rules JSON editor stays concise. */
function compactRules(rules) {
  const out = stripDefaults(rules, {
    win: "all_on_foundations",
    auto_flip: true,
    safe_autoplay: true,
  });
  if (out.pack) {
    const p = { ...out.pack };
    if (p.copies === 1) delete p.copies;
    if (eqArr(p.suits, DEFAULT_SUITS)) delete p.suits;
    if (eqArr(p.ranks, DEFAULT_RANKS)) delete p.ranks;
    if (p.jokers === 0) delete p.jokers;
    if (Object.keys(p).length === 0) delete out.pack;
    else out.pack = p;
  }
  if (Array.isArray(out.groups)) {
    out.groups = out.groups.map(compactGroup);
  }
  if (out.stock) {
    out.stock = stripDefaults(out.stock, {
      count: 1,
      redeals: 0,
      unlimited_redeals: false,
      require_no_empty: false,
      auto: false,
      first_empty: false,
      only_when_empty: false,
      skip_empty: false,
      face_up: false,
    });
  }
  if (Array.isArray(out.deal)) {
    out.deal = out.deal.map((d) =>
      stripDefaults(d, {
        rest: false,
        face_up: true,
        kings_to_bottom: false,
        one_each: false,
        set_base: false,
        of_base: false,
      }),
    );
  }
  return out;
}

function maskFaceDown(st) {
  const piles = {};
  for (const [group, list] of Object.entries(st.piles)) {
    piles[group] = list.map((pile) =>
      pile.map((card) => (card.startsWith("~") ? "?" : card)),
    );
  }
  return { ...st, piles };
}

function parseCard(str) {
  const down = str.startsWith("~") || str === "?";
  const face = str.startsWith("~") ? str.slice(1) : str;
  if (down) return { down: true, text: "", suit: "", red: false, joker: false };
  if (face === "JK") return { down: false, text: "JK", suit: "★", red: false, joker: true };
  const suitChar = face.slice(-1);
  const rankRaw = face.slice(0, -1);
  const rank = rankRaw === "T" ? "10" : rankRaw;
  return {
    down: false,
    text: rank,
    suit: SUIT_SYMBOL[suitChar] || suitChar,
    red: RED_SUITS.has(suitChar),
    joker: false,
  };
}

function renderCardEl(cardStr, topPx) {
  const c = parseCard(cardStr);
  const div = document.createElement("div");
  div.className = "card" + (c.down ? " down" : c.joker ? " joker" : c.red ? " red" : "");
  div.style.top = `${topPx}px`;
  if (!c.down) {
    const label = `${c.text}${c.suit}`;
    const top = document.createElement("span");
    top.textContent = label;
    const center = document.createElement("span");
    center.className = "card-center";
    center.textContent = c.suit;
    const bot = document.createElement("span");
    bot.className = "card-bot";
    bot.textContent = label;
    div.append(top, center, bot);
  }
  return div;
}

function targetsForSelected() {
  if (!selectedPile) return [];
  const out = [];
  for (const st of legalSteps) {
    if (st.type === "move" && samePile(st.from, selectedPile)) {
      out.push({ pile: st.to, step: st });
    } else if (st.type === "remove" && st.piles.length === 2) {
      if (samePile(st.piles[0], selectedPile)) out.push({ pile: st.piles[1], step: st });
      else if (samePile(st.piles[1], selectedPile)) out.push({ pile: st.piles[0], step: st });
    }
  }
  return out;
}

function pileIsSelectable(ref) {
  for (const st of legalSteps) {
    if (st.type === "move" && samePile(st.from, ref)) return true;
    if (st.type === "remove" && st.piles.some((p) => samePile(p, ref))) return true;
    if (
      (st.type === "deal" || st.type === "redeal") &&
      currentRules.groups.find((g) => g.name === ref.group)?.kind === "stock"
    ) {
      return true;
    }
  }
  return false;
}

function pileIsHinted(ref) {
  if (!hintedStep) return false;
  if (hintedStep.type === "move") {
    return samePile(hintedStep.from, ref) || samePile(hintedStep.to, ref);
  }
  if (hintedStep.type === "remove") {
    return hintedStep.piles.some((p) => samePile(p, ref));
  }
  if (hintedStep.type === "deal" || hintedStep.type === "redeal") {
    return currentRules.groups.find((g) => g.name === ref.group)?.kind === "stock";
  }
  return false;
}

async function onPileClick(ref) {
  if (busy) return;
  const grp = currentRules.groups.find((g) => g.name === ref.group);

  // Stock click -> deal / redeal directly
  if (grp?.kind === "stock") {
    const dealStep = legalSteps.find((s) => s.type === "deal" || s.type === "redeal");
    if (dealStep) {
      await applyUserStep(dealStep);
      return;
    }
  }

  // If a pile is selected and user clicked a valid target -> play it!
  if (selectedPile) {
    const matches = targetsForSelected().filter((t) => samePile(t.pile, ref));
    if (matches.length > 0) {
      // If multiple counts move from A -> B, pick the largest run (most intuitive).
      matches.sort((a, b) => (b.step.count || 0) - (a.step.count || 0));
      await applyUserStep(matches[0].step);
      return;
    }
    if (samePile(selectedPile, ref)) {
      selectedPile = null;
      renderBoard();
      return;
    }
  }

  // Single-pile removal (e.g. Pyramid King, Aces Up)
  const soloRemove = legalSteps.find(
    (s) => s.type === "remove" && s.piles.length === 1 && samePile(s.piles[0], ref),
  );
  if (soloRemove && !legalSteps.some((s) => s.type === "move" && samePile(s.from, ref))) {
    await applyUserStep(soloRemove);
    return;
  }

  // Otherwise select this pile if it has outgoing moves
  if (pileIsSelectable(ref)) {
    selectedPile = ref;
    renderBoard();
  } else {
    selectedPile = null;
    renderBoard();
  }
}

function renderGroup(grp) {
  const pilesData = state.piles[grp.name] || [];
  const wrap = document.createElement("div");
  wrap.className = "group";

  const lbl = document.createElement("div");
  lbl.className = "group-label";
  lbl.textContent = grp.name;
  wrap.appendChild(lbl);

  const pilesEl = document.createElement("div");
  const isGrid = Boolean(grp.columns);
  pilesEl.className = isGrid ? "piles grid" : "piles";
  if (isGrid) {
    pilesEl.style.gridTemplateColumns = `repeat(${grp.columns}, var(--card-w))`;
  }

  const fan = grp.kind === "tableau" && !isGrid;
  const targets = targetsForSelected();

  for (let i = 0; i < pilesData.length; i++) {
    const cards = pilesData[i];
    const ref = { group: grp.name, index: i };
    const pileEl = document.createElement("div");
    pileEl.className = "pile";
    if (samePile(selectedPile, ref)) pileEl.classList.add("selected");
    if (targets.some((t) => samePile(t.pile, ref))) pileEl.classList.add("target");
    if (pileIsHinted(ref)) pileEl.classList.add("hinted");
    if (pileIsSelectable(ref)) pileEl.classList.add("selectable");

    if (fan && cards.length > 0) {
      const stepDown = 10;
      const stepUp = 20;
      let y = 0;
      for (let k = 0; k < cards.length; k++) {
        pileEl.appendChild(renderCardEl(cards[k], y));
        if (k + 1 < cards.length) {
          y += cards[k].startsWith("~") ? stepDown : stepUp;
        }
      }
      pileEl.style.height = `calc(var(--card-h) + ${y}px)`;
    } else if (cards.length > 0) {
      pileEl.appendChild(renderCardEl(cards[cards.length - 1], 0));
      if (cards.length > 1) {
        const badge = document.createElement("span");
        badge.className = "pile-count";
        badge.textContent = String(cards.length);
        pileEl.appendChild(badge);
      }
    }

    pileEl.addEventListener("click", () => onPileClick(ref));
    pilesEl.appendChild(pileEl);
  }

  wrap.appendChild(pilesEl);
  return wrap;
}

function renderBoard() {
  elBoard.innerHTML = "";
  if (!state || !currentRules) return;

  const topKinds = new Set(["stock", "waste", "cell", "reserve", "foundation"]);
  const topGroups = currentRules.groups.filter((g) => topKinds.has(g.kind));
  const mainGroups = currentRules.groups.filter((g) => !topKinds.has(g.kind));

  if (topGroups.length > 0) {
    const row = document.createElement("div");
    row.className = "board-row";
    for (const g of topGroups) row.appendChild(renderGroup(g));
    elBoard.appendChild(row);
  }
  if (mainGroups.length > 0) {
    const row = document.createElement("div");
    row.className = "board-row";
    for (const g of mainGroups) row.appendChild(renderGroup(g));
    elBoard.appendChild(row);
  }

  // Legal moves chips
  elLegalTitle.textContent = `Legal moves (${legalSteps.length})`;
  elLegalList.innerHTML = "";
  for (const st of legalSteps) {
    const b = document.createElement("button");
    b.className = "move-chip" + (sameStep(st, hintedStep) ? " hinted" : "");
    b.textContent = formatStep(st);
    b.addEventListener("click", () => {
      if (!busy) applyUserStep(st);
    });
    elLegalList.appendChild(b);
  }
}

async function refreshStateMeta() {
  legalSteps = await worker.legalSteps(state);
  const won = await worker.isWon(state);
  const sc = await worker.score(state);
  btnUndo.disabled = busy || history.length === 0;
  renderBoard();
  if (won) {
    setStatus(sc !== null ? `Won! Final score: ${sc}` : "Won!");
  } else if (sc !== null) {
    setStatus(`Current score: ${sc} · ${legalSteps.length} legal moves`);
  }
}

async function applyUserStep(step) {
  clearPlayback();
  history.push(state);
  selectedPile = null;
  hintedStep = null;
  state = await worker.playStep(state, step);
  await refreshStateMeta();
}

async function respawnWorker(fromCustomRules = false) {
  if (worker) worker.terminate();
  worker = fromCustomRules
    ? await WorkerSolver.fromRules(currentRules)
    : await WorkerSolver.fromPreset(currentPreset);
  currentRules = await worker.rules();
  elRulesEditor.value = JSON.stringify(compactRules(currentRules), null, 2);
  btnSolve.textContent = isScored() ? "Best Score" : "Solve";
  elMsWrap.classList.toggle("hidden", currentPreset !== "freecell");
}

function toBase64Url(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromBase64Url(str) {
  const b64 = str.replace(/-/g, "+").replace(/_/g, "/");
  const pad = b64.length % 4 === 0 ? "" : "=".repeat(4 - (b64.length % 4));
  const bin = atob(b64 + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

async function encodeRulesParam(rulesObj) {
  const bytes = new TextEncoder().encode(JSON.stringify(compactRules(rulesObj)));
  if (typeof CompressionStream === "function") {
    const cs = new CompressionStream("deflate-raw");
    const writer = cs.writable.getWriter();
    writer.write(bytes);
    writer.close();
    const compressed = new Uint8Array(await new Response(cs.readable).arrayBuffer());
    return "z:" + toBase64Url(compressed);
  }
  return "r:" + toBase64Url(bytes);
}

const MAX_RULES_BYTES = 512 * 1024;

async function readStreamWithLimit(readable, maxBytes) {
  const reader = readable.getReader();
  const chunks = [];
  let total = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      total += value.byteLength;
      if (total > maxBytes) {
        await reader.cancel();
        throw new Error(`Rules payload exceeds ${maxBytes} byte limit`);
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out;
}

async function decodeRulesParam(encoded) {
  if (encoded.startsWith("z:") && typeof DecompressionStream === "function") {
    const bytes = fromBase64Url(encoded.slice(2));
    if (bytes.byteLength > MAX_RULES_BYTES) {
      throw new Error(`Rules payload exceeds ${MAX_RULES_BYTES} byte limit`);
    }
    const ds = new DecompressionStream("deflate-raw");
    const writer = ds.writable.getWriter();
    const writePromise = writer.write(bytes).then(() => writer.close());
    const [decompressed] = await Promise.all([
      readStreamWithLimit(ds.readable, MAX_RULES_BYTES),
      writePromise.catch(() => {}),
    ]);
    return JSON.parse(new TextDecoder().decode(decompressed));
  }
  const payload = encoded.startsWith("r:") ? encoded.slice(2) : encoded;
  const bytes = fromBase64Url(payload);
  if (bytes.byteLength > MAX_RULES_BYTES) {
    throw new Error(`Rules payload exceeds ${MAX_RULES_BYTES} byte limit`);
  }
  return JSON.parse(new TextDecoder().decode(bytes));
}

async function syncUrlHash() {
  const params = new URLSearchParams();
  const seed = Math.max(1, Number.parseInt(elSeed.value, 10) || 1);
  if (isCustomRules && currentRules) {
    params.set("rules", await encodeRulesParam(currentRules));
    params.set("seed", String(seed));
  } else {
    params.set("game", currentPreset);
    params.set("seed", String(seed));
    if (currentPreset === "freecell") {
      params.set("ms", elMsMode.checked ? "1" : "0");
    }
  }
  const next = "#" + params.toString();
  if (window.location.hash !== next) {
    window.history.replaceState(null, "", next);
  }
}

async function dealCurrent() {
  clearPlayback();
  history = [];
  selectedPile = null;
  hintedStep = null;
  const n = Math.max(1, Number.parseInt(elSeed.value, 10) || 1);
  elSeed.value = String(n);
  setBusy(true);
  try {
    state =
      !isCustomRules && currentPreset === "freecell" && elMsMode.checked
        ? await worker.dealMicrosoft(n)
        : await worker.deal(n);
    setStatus(`Dealt ${currentRules.name} #${n}`);
    await refreshStateMeta();
    await syncUrlHash();
  } finally {
    setBusy(false);
  }
}

async function loadPreset(id) {
  currentPreset = id;
  isCustomRules = false;
  elRulesError.textContent = "";
  setBusy(true);
  setStatus(`Loading ${id}…`);
  try {
    await respawnWorker(false);
    await dealCurrent();
  } finally {
    setBusy(false);
  }
}

async function runSolve() {
  clearPlayback();
  selectedPile = null;
  hintedStep = null;
  setBusy(true);
  const scored = isScored();
  setStatus(scored ? "Searching for highest score…" : "Solving in Web Worker…");
  const t0 = performance.now();
  try {
    if (scored) {
      const r = await worker.best(state, { max_states: 250_000 });
      const ms = Math.round(performance.now() - t0);
      setStatus(
        `Best score: ${r.score}${r.proven ? " (proven)" : ""} · ${r.states.toLocaleString()} states · ${ms} ms`,
      );
      await setupPlayback(r.steps);
    } else {
      const r = await worker.solve(state, { max_states: 500_000 });
      const ms = Math.round(performance.now() - t0);
      if (r.outcome === "solved") {
        setStatus(
          `Solved in ${r.steps.length} steps · ${r.states.toLocaleString()} states · ${ms} ms`,
        );
        await setupPlayback(r.steps);
      } else if (r.outcome === "unsolvable") {
        setStatus(`Unsolvable (proven) · ${r.states.toLocaleString()} states · ${ms} ms`);
      } else {
        setStatus(`Gave up after ${r.states.toLocaleString()} states · ${ms} ms`);
      }
    }
  } catch (err) {
    if (err.message.includes("terminated")) {
      setStatus("Search cancelled.");
    } else {
      setStatus(`Error: ${err.message}`);
    }
  } finally {
    setBusy(false);
  }
}

async function runHint() {
  clearPlayback();
  selectedPile = null;
  hintedStep = null;
  setBusy(true);
  setStatus("Computing fair hint (face-down cards masked as '?')…");
  const t0 = performance.now();
  try {
    const masked = maskFaceDown(state);
    const h = await worker.hint(masked, {
      samples: 12,
      solve: { max_states: 100_000 },
    });
    const ms = Math.round(performance.now() - t0);
    if (!h) {
      setStatus(`No winning hint found (${ms} ms)`);
    } else {
      hintedStep = h.step;
      const detail =
        h.score !== undefined
          ? `avg score ${h.score.toFixed(1)}`
          : `won ${h.wins}/${h.samples} guesses`;
      setStatus(`Hint: ${formatStep(h.step)} (${detail}, ${ms} ms)`);
      renderBoard();
    }
  } catch (err) {
    if (err.message.includes("terminated")) {
      setStatus("Hint cancelled.");
    } else {
      setStatus(`Error: ${err.message}`);
    }
  } finally {
    setBusy(false);
  }
}

async function setupPlayback(steps) {
  if (!steps || steps.length === 0) return;
  playbackSteps = steps;
  playbackStates = [state];
  let cur = state;
  for (const st of steps) {
    cur = await worker.playStep(cur, st);
    playbackStates.push(cur);
  }
  playbackIdx = 0;
  elPlayback.classList.remove("hidden");
  updatePlaybackView();
}

function updatePlaybackView() {
  state = playbackStates[playbackIdx];
  hintedStep = playbackIdx < playbackSteps.length ? playbackSteps[playbackIdx] : null;
  elStepCounter.textContent = `${playbackIdx} / ${playbackSteps.length}`;
  btnFirst.disabled = playbackIdx === 0;
  btnPrev.disabled = playbackIdx === 0;
  btnNext.disabled = playbackIdx >= playbackSteps.length;
  renderBoard();
}

elPreset.addEventListener("change", () => loadPreset(elPreset.value));
btnDeal.addEventListener("click", () => dealCurrent());
btnSolve.addEventListener("click", () => runSolve());
btnHint.addEventListener("click", () => runHint());

btnCancel.addEventListener("click", async () => {
  if (!busy) return;
  setStatus("Cancelling…");
  await respawnWorker(true);
  setBusy(false);
  setStatus("Cancelled.");
});

btnUndo.addEventListener("click", async () => {
  if (busy || history.length === 0) return;
  clearPlayback();
  state = history.pop();
  selectedPile = null;
  hintedStep = null;
  await refreshStateMeta();
});

btnFirst.addEventListener("click", () => {
  stopPlayback();
  playbackIdx = 0;
  updatePlaybackView();
});

btnPrev.addEventListener("click", () => {
  stopPlayback();
  if (playbackIdx > 0) {
    playbackIdx--;
    updatePlaybackView();
  }
});

btnNext.addEventListener("click", () => {
  stopPlayback();
  if (playbackIdx < playbackSteps.length) {
    playbackIdx++;
    updatePlaybackView();
  }
});

btnPlay.addEventListener("click", () => {
  if (playTimer !== null) {
    stopPlayback();
    return;
  }
  if (playbackIdx >= playbackSteps.length) playbackIdx = 0;
  btnPlay.textContent = "⏸ Pause";
  playTimer = setInterval(() => {
    if (playbackIdx < playbackSteps.length) {
      playbackIdx++;
      updatePlaybackView();
    } else {
      stopPlayback();
    }
  }, 220);
});

btnApplyRules.addEventListener("click", async () => {
  elRulesError.textContent = "";
  setBusy(true);
  try {
    const parsed = JSON.parse(elRulesEditor.value);
    const nextWorker = await WorkerSolver.fromRules(parsed);
    if (worker) worker.terminate();
    worker = nextWorker;
    isCustomRules = true;
    currentRules = await worker.rules();
    btnSolve.textContent = isScored() ? "Best Score" : "Solve";
    elMsWrap.classList.add("hidden");
    await dealCurrent();
    setStatus(`Applied custom rules: ${currentRules.name}`);
  } catch (err) {
    elRulesError.textContent = err.message;
  } finally {
    setBusy(false);
  }
});

btnResetRules.addEventListener("click", () => loadPreset(currentPreset));

btnShare.addEventListener("click", async () => {
  await syncUrlHash();
  const url = window.location.href;
  try {
    await navigator.clipboard.writeText(url);
    setStatus("Copied shareable link to clipboard!");
  } catch {
    window.prompt("Copy shareable link:", url);
  }
});

async function boot() {
  try {
    await init();
    const list = presets();
    const params = new URLSearchParams(window.location.hash.replace(/^#/, ""));
    const hashGame = params.get("game");
    const hashSeed = params.get("seed");
    const hashMs = params.get("ms");
    const hashRules = params.get("rules");

    if (hashGame && list.some((p) => p.id === hashGame)) {
      currentPreset = hashGame;
    }
    if (hashSeed && Number.parseInt(hashSeed, 10) >= 1) {
      elSeed.value = String(Number.parseInt(hashSeed, 10));
    }
    if (hashMs === "0") {
      elMsMode.checked = false;
    } else if (hashMs === "1") {
      elMsMode.checked = true;
    }

    for (const p of list) {
      const opt = document.createElement("option");
      opt.value = p.id;
      opt.textContent = p.name;
      if (p.id === currentPreset) opt.selected = true;
      elPreset.appendChild(opt);
    }

    if (hashRules) {
      try {
        const parsed = await decodeRulesParam(hashRules);
        worker = await WorkerSolver.fromRules(parsed);
        isCustomRules = true;
        currentRules = await worker.rules();
        elRulesEditor.value = JSON.stringify(compactRules(currentRules), null, 2);
        btnSolve.textContent = isScored() ? "Best Score" : "Solve";
        elMsWrap.classList.add("hidden");
        await dealCurrent();
        return;
      } catch (err) {
        elRulesError.textContent = `Invalid shared rules in URL: ${err.message}`;
      }
    }

    await loadPreset(currentPreset);
  } catch (err) {
    setStatus(`Failed to initialize WASM: ${err.message}`);
  }
}

boot();
