"""Dealbreaker: fast data-driven patience (solitaire) solver."""

from __future__ import annotations

import json
from typing import Any, Literal, Mapping, Sequence, TypedDict

from ._dealbreaker import CancelToken, _Solver, format_step_json, presets_json

__all__ = [
    "CancelToken",
    "DealStep",
    "Difficulty",
    "GradeThresholds",
    "HintResult",
    "MaxResult",
    "MoveStep",
    "PileRef",
    "PresetInfo",
    "RedealStep",
    "RemoveStep",
    "SolveResult",
    "Solver",
    "StateData",
    "Step",
    "format_step",
    "presets",
]


class PresetInfo(TypedDict):
    """Metadata for a built-in game preset."""

    id: str
    name: str


class PileRef(TypedDict):
    """Reference to a single pile by group name and 0-based index."""

    group: str
    index: int


MoveStep = TypedDict(
    "MoveStep",
    {
        "type": Literal["move"],
        "from": PileRef,
        "to": PileRef,
        "count": int,
    },
)

DealStep = TypedDict("DealStep", {"type": Literal["deal"]})
RedealStep = TypedDict("RedealStep", {"type": Literal["redeal"]})
RemoveStep = TypedDict("RemoveStep", {"type": Literal["remove"], "piles": list[PileRef]})

Step = MoveStep | DealStep | RedealStep | RemoveStep


class _StateDataOptional(TypedDict, total=False):
    redeals_used: int
    base_rank: int
    rng_state: int


class StateData(_StateDataOptional):
    """Serializable game position."""

    piles: dict[str, list[list[str]]]


class _SolveResultOptional(TypedDict, total=False):
    steps: list[Step]


class SolveResult(_SolveResultOptional):
    """Result returned by :meth:`Solver.solve`."""

    outcome: Literal["solved", "unsolvable", "gave-up"]
    states: int


class MaxResult(TypedDict):
    """Result returned by :meth:`Solver.best`."""

    score: int
    proven: bool
    states: int
    steps: list[Step]


class _HintResultOptional(TypedDict, total=False):
    score: float


class HintResult(_HintResultOptional):
    """Result returned by :meth:`Solver.hint`."""

    step: Step
    wins: int
    samples: int


class GradeThresholds(TypedDict):
    """Score cut-offs: below ``medium`` is easy, below ``hard`` is medium, otherwise hard."""

    medium: float
    hard: float


class _DifficultyOptional(TypedDict, total=False):
    score: float


class Difficulty(_DifficultyOptional):
    """Result returned by :meth:`Solver.grade`.

    ``winnable`` is ``True`` (won), ``False`` (proven unwinnable) or ``None`` (budget ran out);
    anything but ``True`` is graded ``"hard"``. ``score`` is present for winnable deals.
    """

    grade: Literal["easy", "medium", "hard"]
    winnable: bool | None
    effort: int
    probes: int
    traps: int
    trap_rate: float
    steps: int
    thresholds: GradeThresholds
    source: Literal["preset", "custom", "generic"]


def _as_json(value: Mapping[str, Any] | str) -> str:
    return value if isinstance(value, str) else json.dumps(value)


def presets() -> list[PresetInfo]:
    """Return all built-in game presets as ``[{"id": ..., "name": ...}, ...]``."""
    return json.loads(presets_json())


def format_step(step: Step | Mapping[str, Any] | str) -> str:
    """Format a step dict or JSON string as human-readable text (e.g. ``"tableau 4 -> cell 1"``)."""
    return format_step_json(_as_json(step))


class Solver:
    """Compiled solitaire game rules. Create once and reuse across many deals.

    Long-running searches (`solve`, `best`, `hint`) release the Python GIL so
    multiple threads can search concurrently.
    """

    def __init__(self, rules: Mapping[str, Any] | str) -> None:
        self._inner = _Solver(_as_json(rules))

    @classmethod
    def from_preset(cls, preset_id: str) -> Solver:
        """Create a solver from a built-in preset id (see :func:`presets`)."""
        obj = cls.__new__(cls)
        obj._inner = _Solver.from_preset(preset_id)
        return obj

    def rules(self) -> dict[str, Any]:
        """Return this game's rules as a dictionary."""
        return json.loads(self._inner.rules_json())

    def deal(self, seed: int = 1) -> StateData:
        """Deal a seeded position."""
        return json.loads(self._inner.deal_json(seed))

    def deal_microsoft(self, n: int) -> StateData:
        """Deal Microsoft FreeCell deal ``n`` (for FreeCell-layout games)."""
        return json.loads(self._inner.deal_microsoft_json(n))

    def deal_ordered(self, cards: Sequence[str]) -> StateData:
        """Deal from an explicit list of card strings (e.g. ``["AS", "2S", ...]``)."""
        return json.loads(self._inner.deal_ordered_json(list(cards)))

    def validate(self, state: StateData | Mapping[str, Any] | str) -> None:
        """Validate a position against this game's pack and pile capacities."""
        self._inner.validate_json(_as_json(state))

    def solve(
        self,
        state: StateData | Mapping[str, Any] | str,
        *,
        max_states: int = 2_000_000,
        strategy: Literal["auto", "depth_first", "depth-first", "dfs", "best_first", "best-first", "bfs"] = "auto",
        timeout_ms: int | None = None,
        autoplay: bool = True,
        shorten: bool = True,
        cancel: CancelToken | None = None,
    ) -> SolveResult:
        """Search for a winning line or prove unsolvability. Releases the GIL."""
        return json.loads(
            self._inner.solve_json(
                _as_json(state),
                max_states=max_states,
                strategy=strategy,
                timeout_ms=timeout_ms,
                autoplay=autoplay,
                shorten=shorten,
                cancel=cancel,
            )
        )

    def best(
        self,
        state: StateData | Mapping[str, Any] | str,
        *,
        max_states: int = 2_000_000,
        strategy: Literal["auto", "depth_first", "depth-first", "dfs", "best_first", "best-first", "bfs"] = "auto",
        timeout_ms: int | None = None,
        cancel: CancelToken | None = None,
    ) -> MaxResult:
        """Find the highest-scoring line in a points game. Releases the GIL."""
        return json.loads(
            self._inner.best_json(
                _as_json(state),
                max_states=max_states,
                strategy=strategy,
                timeout_ms=timeout_ms,
                cancel=cancel,
            )
        )

    def hint(
        self,
        state: StateData | Mapping[str, Any] | str,
        *,
        samples: int = 16,
        seed: int = 0,
        max_states: int = 100_000,
        strategy: Literal["auto", "depth_first", "depth-first", "dfs", "best_first", "best-first", "bfs"] = "auto",
        timeout_ms: int | None = None,
        autoplay: bool = True,
        shorten: bool = True,
        cancel: CancelToken | None = None,
    ) -> HintResult | None:
        """Suggest a next step without peeking at face-down cards. Releases the GIL."""
        return json.loads(
            self._inner.hint_json(
                _as_json(state),
                samples=samples,
                seed=seed,
                max_states=max_states,
                strategy=strategy,
                timeout_ms=timeout_ms,
                autoplay=autoplay,
                shorten=shorten,
                cancel=cancel,
            )
        )

    def grade(
        self,
        state: StateData | Mapping[str, Any] | str,
        *,
        max_states: int = 500_000,
        probe_positions: int = 10,
        probe_states: int = 3_000,
        thresholds: tuple[float, float] | None = None,
        cancel: CancelToken | None = None,
    ) -> Difficulty:
        """Grade a deal as ``"easy"``, ``"medium"`` or ``"hard"``. Releases the GIL.

        Built-in thresholds are calibrated for each preset with the default budgets; pass
        ``thresholds=(medium, hard)`` to use your own (e.g. from ``dealbreaker grade --calibrate``).
        Unwinnable deals, and deals not won within ``max_states``, are graded ``"hard"``.
        """
        return json.loads(
            self._inner.grade_json(
                _as_json(state),
                max_states=max_states,
                probe_positions=probe_positions,
                probe_states=probe_states,
                thresholds=thresholds,
                cancel=cancel,
            )
        )

    def legal_steps(self, state: StateData | Mapping[str, Any] | str) -> list[Step]:
        """Return every legal single step in ``state``."""
        return json.loads(self._inner.legal_steps_json(_as_json(state)))

    def play_step(
        self,
        state: StateData | Mapping[str, Any] | str,
        step: Step | Mapping[str, Any] | str,
    ) -> StateData:
        """Play one step and return the resulting position (raises ValueError if illegal)."""
        return json.loads(self._inner.play_step_json(_as_json(state), _as_json(step)))

    def is_won(self, state: StateData | Mapping[str, Any] | str) -> bool:
        """Return whether ``state`` is a won position."""
        return self._inner.is_won_json(_as_json(state))

    def score(self, state: StateData | Mapping[str, Any] | str) -> int | None:
        """Return points for ``state``, or ``None`` for non-scored games."""
        return self._inner.score_json(_as_json(state))

    def render(self, state: StateData | Mapping[str, Any] | str) -> str:
        """Render ``state`` as a human-readable ASCII board."""
        return self._inner.render_json(_as_json(state))
