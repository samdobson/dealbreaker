# Built-in Games

Dealbreaker ships with **39 built-in patience (solitaire) presets**—ready to play, solve, or customize.

| Family | Presets |
|---|---|
| **Klondike** | `klondike`, `klondike-draw3`, `klondike-vegas`, `klondike-jokers` |
| **FreeCell** | `freecell`, `bakers-game`, `eight-off`, `seahaven-towers` |
| **Spider & Yukon** | `spider`, `spider-2suit`, `spider-1suit`, `spiderette`, `scorpion`, `yukon`, `russian` |
| **Tableau & Reserve** | `forty-thieves`, `gypsy`, `bakers-dozen`, `beleaguered-castle`, `streets-and-alleys`, `fortress`, `canfield`, `bisley`, `cruel`, `la-belle-lucie` |
| **Pairing & Removal** | `golf`, `tripeaks`, `pyramid`, `aces-up`, `elevens`, `nestor`, `monte-carlo` |
| **Grid & Other** | `calculation`, `sir-tommy`, `gaps-single-deal`, `montana`, `accordion` |
| **Points** | `poker-squares`, `cribbage-squares` |

## Inspect or Customize Any Preset

Every built-in game is just a rules definition. Print one out to see how it works or use it as a template for your own variant:

```sh
dealbreaker list                # list all 39 presets with display names
dealbreaker rules freecell      # dump FreeCell's JSON rules
```

Then tweak the JSON and pass the file path anywhere a preset name is accepted—see [Custom Rules (JSON)](rules-format.md).
