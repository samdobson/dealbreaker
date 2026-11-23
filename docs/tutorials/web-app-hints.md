# Add Fair Hints to a Web App

Running a solver directly on the browser's main thread will freeze animations while it searches. In this tutorial, you'll use `WorkerSolver` (`dealbreaker/worker`) to run Dealbreaker in a Web Worker, request a fair hint, play the move, and support instant cancellation.

## 1. Install the Package

```sh
npm install dealbreaker
```

Dealbreaker ships as zero-dependency ESM + a ~600 KB WebAssembly binary and works out of the box with Vite, webpack, esbuild, and modern browsers.

## 2. Start a `WorkerSolver`

Create a worker-backed solver for Klondike and deal seed `42`:

```ts
import { WorkerSolver } from "dealbreaker/worker";

const solver = await WorkerSolver.fromPreset("klondike");
let state = await solver.deal(42);
```

Every method on `WorkerSolver` mirrors `Solver`, except it returns a `Promise` and runs off the main thread.

## 3. Ask for a Fair Hint

When the player clicks **Hint**, call `hint()`:

```ts
const suggestion = await solver.hint(state, {
  samples: 16,
  solve: { max_states: 100_000 },
});

if (suggestion) {
  console.log(
    `Recommended move (${suggestion.wins}/${suggestion.samples} samples won):`,
    suggestion.step
  );
  // Apply the move to get the updated position
  state = await solver.playStep(state, suggestion.step);
} else {
  console.log("No winning move found within budget.");
}
```

Because `hint()` masks face-down cards (and any `"?"` placeholders you pass in), it never cheats or leaks hidden information.

## 4. Cancel an In-Flight Search

If the player clicks **Cancel** or starts a new deal while a search is running, terminate the worker immediately and spin up a fresh one:

```ts
function cancelSearch() {
  solver.terminate();
}
```

## 5. Inspect Legal Moves for Your UI

You don't need to re-implement solitaire move legality in JavaScript—ask Dealbreaker directly:

```ts
const moves = await solver.legalSteps(state);
const won = await solver.isWon(state);
```

## Where to Go Next

- **Live example:** Check out the <a href="/dealbreaker/demo/" target="_self">Live Web Demo</a> (built with `WorkerSolver` in plain HTML/JS)
- **Reference:** [JavaScript / TypeScript Reference](/reference/javascript)
- **How-to:** [Configure Fair Hints](/how-to/fair-hints)
- **Explanation:** [Hidden Cards & Fair Hints](/explanation/hidden-cards)
