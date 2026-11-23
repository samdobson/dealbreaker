"""Dealbreaker: fast data-driven patience (solitaire) solver."""

from __future__ import annotations

import json
from typing import Any, Mapping, Sequence

from ._dealbreaker import _Solver, presets_json

__all__ = ["Solver", "presets"]


def _as_json(value: Mapping[str, Any] | str) -> str:
    return value if isinstance(value, str) else json.dumps(value)


def presets() -> list[dict[str, str]]:
    """Return all built-in game presets as ``[{"id": ..., "name": ...}, ...]``."""
    return json.loads(presets_json())


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

    def deal(self, seed: int = 1) -> dict[str, Any]:
        """Deal a seeded position."""
        return json.loads(self._inner.deal_json(seed))

    def deal_microsoft(self, n: int) -> dict[str, Any]:
        """Deal Microsoft FreeCell deal ``n`` (for FreeCell-layout games)."""
        return json.loads(self._inner.deal_microsoft_json(n))

    def deal_ordered(self, cards: Sequence[str]) -> dict[str, Any]:
        """Deal from an explicit list of card strings (e.g. ``["AS", "2S", ...]``)."""
        return json.loads(self._inner.deal_ordered_json(list(cards)))

    def validate(self, state: Mapping[str, Any] | str) -> None:
        """Validate a position against this game's pack and pile capacities."""
        self._inner.validate_json(_as_json(state))

    def solve(
        self,
        state: Mapping[str, Any] | str,
        *,
        max_states: int = 2_000_000,
        strategy: str = "auto",
        timeout_ms: int | None = None,
        autoplay: bool = True,
        shorten: bool = True,
    ) -> dict[str, Any]:
        """Search for a winning line or prove unsolvability. Releases the GIL."""
        return json.loads(
            self._inner.solve_json(
                _as_json(state),
                max_states=max_states,
                strategy=strategy,
                timeout_ms=timeout_ms,
                autoplay=autoplay,
                shorten=shorten,
            )
        )

    def best(
        self,
        state: Mapping[str, Any] | str,
        *,
        max_states: int = 2_000_000,
        strategy: str = "auto",
        timeout_ms: int | None = None,
    ) -> dict[str, Any]:
        """Find the highest-scoring line in a points game. Releases the GIL."""
        return json.loads(
            self._inner.best_json(
                _as_json(state),
                max_states=max_states,
                strategy=strategy,
                timeout_ms=timeout_ms,
            )
        )

    def hint(
        self,
        state: Mapping[str, Any] | str,
        *,
        samples: int = 16,
        seed: int = 0,
        max_states: int = 100_000,
        strategy: str = "auto",
        timeout_ms: int | None = None,
        autoplay: bool = True,
        shorten: bool = True,
    ) -> dict[str, Any] | None:
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
            )
        )

    def legal_steps(self, state: Mapping[str, Any] | str) -> list[dict[str, Any]]:
        """Return every legal single step in ``state``."""
        return json.loads(self._inner.legal_steps_json(_as_json(state)))

    def play_step(
        self,
        state: Mapping[str, Any] | str,
        step: Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Play one step and return the resulting position (raises ValueError if illegal)."""
        return json.loads(self._inner.play_step_json(_as_json(state), _as_json(step)))

    def is_won(self, state: Mapping[str, Any] | str) -> bool:
        """Return whether ``state`` is a won position."""
        return self._inner.is_won_json(_as_json(state))

    def score(self, state: Mapping[str, Any] | str) -> int | None:
        """Return points for ``state``, or ``None`` for non-scored games."""
        return self._inner.score_json(_as_json(state))
