# Quotail — visual contract

Character-exact reference renders. **Do not redesign these.**
Every screen is exactly 96 columns wide. Candle/volume cells below are
placeholders — the *frame* (borders, labels, column positions) is the contract.

These blocks are captured at **96x31** (the snapshot target). The layout is
**vertically responsive**: the bottom bar is always the last two rows, and the
charts absorb extra height — see the notes per screen. Snapshot tests assert the
frame at 96x31; separate tests check the responsive behavior at other sizes.

## Overview

96x31. Left panel 40 cols, chart grid 56 cols. The grid is a 2x2 of 28-wide
panes that **stretch vertically** to fill the rows between banner and bar; at
96x31 each pane is 28x13. Watchlist and grid share that middle region.

```
┌─ hot movers ─────────────────────────────────────────────────────────────────────────────────┐
│ ▲ NVDA +4.03%      ▲ UNH +2.05%      ▼ LLY -1.44%      ▲ AVGO +1.12%      ▼ MSFT -0.97%      │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
┌─ watchlist ─ [All] Stk Cry Idx ──────┐┌─ NVDA 210.96 +4.03% ─────┐┌─ BTC-USD ────────────────┐
│ ticker          price     chg% ▼     ││                        │ ││                        │ │
│ NVDA           210.96   +4.03%       ││                    ██│██ ││                    ██│██ │
│ UNH            529.44   +2.05%       ││                ││██████  ││                ││██████  │
│ AVGO           182.44   +1.12%       ││             ██│████      ││             ██│████      │
│ JPM            264.90   +0.62%       ││          ││█████         ││          ││█████         │
│ META           620.80   +0.43%       ││         ████│            ││         ████│            │
│ TSLA           407.76   +0.30%       ││      ████│█│             ││      ████│█│             │
│ V              341.20   +0.19%       ││   ││███││                ││   ││███││                │
│ AMZN           238.18   +0.11%       ││  ████                    ││  ████                    │
│ BRK-B          498.10   +0.08%       ││ ██ │                     ││ ██ │                     │
│ AAPL           315.32   -0.28%       ││ █│                       ││ █│                       │
│ XOM            118.77   -0.31%       │└────────── 1M ────────────┘└────────── 1M ────────────┘
│ GOOGL          357.18   -0.48%       │┌─ AAPL 315.32 -0.28% ─────┐┌──────────────────────────┐
│ MSFT           456.66   -0.97%       ││                        │ ││                          │
│ LLY            812.35   -1.44%       ││                    ██│██ ││                          │
│                                      ││                ││██████  ││                          │
│                                      ││             ██│████      ││                          │
│                                      ││          ││█████         ││                          │
│                                      ││         ████│            ││      enter to chart      │
│                                      ││      ████│█│             ││                          │
│                                      ││   ││███││                ││                          │
│                                      ││  ████                    ││                          │
│                                      ││ ██ │                     ││                          │
│                                      ││ █│                       ││                          │
└──────────────────────────────────────┘└────────── 1M ────────────┘└──────────────────────────┘
 NYSE: Closed                                      [1M]                  Last Refresh: 22:20:01 
 q quit  d detail  / search  f filter  s sort  c clear  tab pane  r refresh  : cmd  ? help      
```

## Detail

96x31. Left column 64 cols, fundamentals rail 32 cols. The **main chart absorbs
slack**; the volume (5) and rsi (6) strips stay fixed-height. At 96x31 the main
chart is rows 0-17 (16 body rows).

```
┌─ AAPL · Apple Inc. · $326.77  +1.24 (+0.38%) ────────────────┐┌─ fundamentals ───────────────┐
│                                                   ██         ││                              │
│                                           ███│  ████         ││ market cap           $4.71 T │
│                                    ││   │█████████           ││ p/e (ttm)               32.4 │
│                                   █████ ██   │█││            ││ p/e (fwd)               28.1 │
│                            ███  │██ │████                    ││ eps (ttm)               9.73 │
│                      │   │████████              ····         ││ div yield              0.42% │
│                   │████████  │ │           ·····             ││ beta                    1.24 │
│             │█   ███  ███             ·····                  ││                              │
│            ███████                ····                       ││ day range                    │
│      ██│ │██  │██│          ······                           ││ 323.50 ───────●────── 330.04 │
│    ████████              ···                                 ││                              │
│ ││██   ││           ·····                                    ││ 52-wk range                  │
│ ███            ·····                         ·······         ││ 196.06 ────────────●─ 343.11 │
│ ·············································                ││                              │
│     ······                                                   ││ indicators                   │
│ ····                                                         ││ ma50                  424.02 │
└── ma50 ·── ma200 ·──────────────────────────────── 1M ───────┘│ ma200                 367.68 │
┌─ volume  1.0M · avg 1.2M ────────────────────────────────────┐│ rsi (14)                68.2 │
│                        █    █     █     █     █    █         ││                              │
│    █ █  █  █ █   █ █   ██   ██    ██    █     █    █         ││ session                      │
│ ████████████████████████████████████████████████████         ││ open                  326.77 │
└──────────────────────────────────────────────────────────────┘│ prev close            326.77 │
┌─ rsi (14)  68.2 ─────────────────────────────────────────────┐│ day high              330.04 │
│ ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●─●●●●●         ││ day low               323.50 │
│                                               ●              ││                              │
│                                                              ││                              │
│ ────────────────────────────────────────────────────         ││                              │
└──────────────────────────────────────────────────────────────┘└──────────────────────────────┘
 NASDAQ: Closed                                    [1M]                  Last Refresh: 22:20:01 
 esc back  / search  1-7 timeframe  e export  r refresh  : cmd  ? help  q quit                  
```

## Help overlay

76-col box, centered (x=10), drawn over dimmed Overview. The dimmed screen shows
through outside the box — that is intended overlay behavior.

```
          ┌─ help ─ ? or esc to close ───────────────────────────────────────────────┐          
          │                                                                          │          
          │  navigation                       chart grid                             │          
          │  d         open detail            enter     chart selected               │          
          │  esc       back to overview       tab       cycle pane focus             │          
          │  /         search ticker          S-1..4    focus pane n                 │          
          │  :         command mode           c         clear focused pane           │          
          │  ?         toggle this help       C         clear all panes              │          
          │  q         quit                   h / l     table <-> grid               │          
          │                                                                          │          
          │  watchlist                        timeframes                             │          
          │  j / k     move selection         1  2  3   1D    5D    1M               │          
          │  g / G     jump top / bottom      4  5      6M    YTD                    │          
          │  f         cycle filter           6  7      1Y    MAX                    │          
          │  s / S     sort / reverse                                                │          
          │  x         remove symbol                                                 │          
          │  r         refresh data                                                  │          
          │                                                                          │          
          │  commands                                                                │          
          │  :add <SYM>      add to watchlist   :tf <RANGE>      set timeframe       │          
          │  :rm <SYM>       remove symbol      :export [path]   write JSON          │          
          │  :detail <SYM>   open drilldown     :settings        open settings       │          
          │                                                                          │          
          └──────────────────────────────────────────────────────────────────────────┘          
```

## Settings

96 cols, full width. Fixed-height content, top-aligned; the bar is anchored to
the floor with blank slack between (rows 26-28 at 96x31).

```
┌─ settings ─ ~/.config/quotail/config.toml ───────────────────────────────────────────────────┐
│                                                                                              │
│  watchlist                                                                                   │
│  stocks              0 symbols                   enter to edit  ·  top 50 by index weight    │
│  crypto              0 symbols                   enter to edit                               │
│  indices             0 symbols                   enter to edit                               │
│                                                                                              │
│  display                                                                                     │
│  theme               tokyonight                  < >  tokyonight, gruvbox, catppuccin, nord  │
│  default_timeframe   1M                          < >  1D 5D 1M 6M YTD 1Y MAX                 │
│  default_filter      all                         < >  all, stocks, crypto, indices           │
│  default_sort        change_pct                  < >  symbol, price, change_pct, mkt_cap     │
│  ticker_scroll       true                        space to toggle  ·  speed 140ms             │
│                                                                                              │
│  data                                                                                        │
│  provider            yahoo                       read-only in MVP                            │
│  poll_interval_sec   60                          < >  15, 30, 60, 300                        │
│  cache_ttl_sec       30                          < >  10, 30, 60                             │
│                                                                                              │
│  mcp                                                                                         │
│  enabled             true                        space to toggle                             │
│  socket_path         …state/quotail/quotail.sock enter to edit                               │
│                                                                                              │
│  w writes changes to config.toml                                                             │
│                                                                                              │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
                                                                                                
                                                                                                
                                                                                                
 NYSE: Closed                                                            Last Refresh: 22:20:01 
 j/k move  enter edit  < > change  space toggle  w write  esc back  ? help  q quit              
```
