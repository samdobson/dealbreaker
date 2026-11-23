import ast, glob, os, json
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

OUT = os.environ.get("OUT","docs")
HERE = os.environ.get("RESULTS",".")
GAMES = [("klondike", "Klondike (draw 1)"), ("klondike-draw3", "Klondike (draw 3)"),
         ("freecell", "FreeCell"), ("bakers-game", "Baker's Game")]
LIMIT = 10.0
DB, SV = "#2a6fdb", "#e0832b"

def load(g):
    rows = []
    for l in open(f"{HERE}/{g}.txt"):
        s, rest = l.split(" ", 1)
        a, b = ast.literal_eval("[" + rest.replace(") (", "),(") + "]")
        rows.append((int(s), a, b))
    return rows

data = {g: load(g) for g, _ in GAMES}
stats = {}
for g, name in GAMES:
    r = data[g]
    n = len(r)
    d = [x[1] for x in r]; s = [x[2] for x in r]
    mism = sum(1 for a, b in zip(d, s) if a[0] != "timeout" and b[0] != "timeout" and a[0] != b[0])
    stats[g] = dict(
        n=n, mismatches=mism,
        db_res=sum(a[0] != "timeout" for a in d), sv_res=sum(b[0] != "timeout" for b in s),
        db_solved=sum(a[0] == "solved" for a in d), sv_solved=sum(b[0] == "solved" for b in s),
        db_unsolv=sum(a[0] == "unsolvable" for a in d), sv_unsolv=sum(b[0] == "unsolvable" for b in s),
        both=sum(a[0] != "timeout" and b[0] != "timeout" for a, b in zip(d, s)),
        db_med=float(np.median([a[1] for a in d])), sv_med=float(np.median([b[1] for b in s])),
    )
json.dump(stats, open(f"{HERE}/stats.json", "w"), indent=1)
for g, s in stats.items(): print(g, s)

plt.rcParams.update({"font.family": "sans-serif", "font.size": 10, "axes.spines.top": False,
                     "axes.spines.right": False, "axes.grid": True, "grid.alpha": .25})

# 1. resolved deals (answered within the limit)
fig, ax = plt.subplots(figsize=(7.5, 3.8))
x = np.arange(len(GAMES)); w = .38
dbv = [100 * stats[g]["db_res"] / stats[g]["n"] for g, _ in GAMES]
svv = [100 * stats[g]["sv_res"] / stats[g]["n"] for g, _ in GAMES]
b1 = ax.bar(x - w / 2, dbv, w, label="Dealbreaker", color=DB)
b2 = ax.bar(x + w / 2, svv, w, label="Solvitaire", color=SV)
for bars in (b1, b2):
    for b in bars: ax.text(b.get_x() + b.get_width() / 2, b.get_height() + 1, f"{b.get_height():.0f}", ha="center", fontsize=9)
ax.set_xticks(x); ax.set_xticklabels([n for _, n in GAMES])
ax.set_ylabel(f"deals answered within {LIMIT:.0f} s (%)"); ax.set_ylim(0, 112)
ax.set_title("Deals resolved (solved or proven unsolvable)", loc="left"); ax.legend(frameon=False, loc="lower left")
fig.tight_layout(); fig.savefig(f"{OUT}/bench-resolved.svg"); plt.close(fig)

# 2. cumulative: deals resolved vs time
fig, axs = plt.subplots(1, len(GAMES), figsize=(11, 2.9), sharey=True)
ts = np.logspace(-3, np.log10(LIMIT), 200)
for ax, (g, name) in zip(axs, GAMES):
    for idx, col, lab in ((1, DB, "Dealbreaker"), (2, SV, "Solvitaire")):
        t = np.array([x[idx][1] for x in data[g] if x[idx][0] != "timeout"])
        ax.plot(ts, [100 * (t <= q).sum() / len(data[g]) for q in ts], color=col, label=lab, lw=2)
    ax.set_xscale("log"); ax.set_title(name, fontsize=10); ax.set_xlabel("time per deal (s)")
    ax.set_ylim(0, 102)
axs[0].set_ylabel("deals resolved (%)"); axs[0].legend(frameon=False, loc="lower right")
fig.tight_layout(); fig.savefig(f"{OUT}/bench-time.svg"); plt.close(fig)

