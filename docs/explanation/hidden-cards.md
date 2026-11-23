# Hidden Cards & Fair Hints

Solitaire has two very different questions you might ask a solver, and Dealbreaker separates them cleanly:

## `solve`: "Can this deal be won?"

`solve` plays the **"thoughtful" (open-information)** game: it sees face-down cards and knows the deal's `seed` (so shuffling redeals in games like La Belle Lucie or Montana are deterministic).

This is what you want when:
- Checking whether a newly generated deal is winnable before showing it to a player.
- Studying win rates of a solitaire variant across thousands of deals.
- Proving a deal is genuinely impossible (`unsolvable`).

## `hint`: "What should a fair player do next?"

`hint` never cheats. Before searching:
1. It masks all face-down cards (and any `"?"` placeholders) and treats future shuffles as unknown.
2. It generates `samples` random valid arrangements of the unseen cards.
3. It solves each sample independently and recommends the first step that wins most often (or achieves the highest average score in points games).

Because every sample is independent, `hint` can also run samples in parallel on native targets (Rust with `parallel` feature, CLI, Python).

See [Configure Fair Hints & Cancellation](/how-to/fair-hints) for tuning sample counts and timeouts.
