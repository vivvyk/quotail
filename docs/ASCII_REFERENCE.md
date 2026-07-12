# Quotail — visual contract

Character-exact reference renders. **Do not redesign these.**
Every screen is exactly 96 columns wide. Candle/volume cells below are
placeholders — the *frame* (borders, labels, column positions) is the contract.

## Overview

96x31. Left panel 40 cols, chart grid 56 cols (2x2 of 28x11).

```
┌─ hot movers ─────────────────────────────────────────────────────────────────────────────────┐
│  ▲ NVDA +4.03%   ▲ AMD +2.71%   ▲ UNH +2.05%   ▼ SOL -1.35%   ▼ LLY -1.44%                   │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
┌─ watchlist ─ [All] Stk Cry Idx ──────┐┌─ NVDA 210.96 +4.03% ─────┐┌─ BTC-USD 63972 -0.24% ───┐
│ ticker          price     chg% ▼     ││ █│ █ │││ │█ │ │││  │█ █│ ││    ██  ██│█│ │ █│ │ │█││ │
│ AAPL           315.32   -0.28%      █││  █    │ │ █│  │█│││███ █ ││ │█  █│█│││█│███ ████││█│ │
│ MSFT           456.66   -0.97%      █││ │█ ││  █  █  █  │││ ███│ ││  │█││ │█│██│█│  │ │█│█││ │
│ NVDA           210.96   +4.03%      █││ ││││ │█ ││ ██│   █ │ │ █ ││ │█ ││█│ │ ││ ██ █│█│██ │ │
│ AMZN           238.18   +0.11%      █││ ││█│  │ █ ││││ ██│█ █││█ ││ │││ │││█ █││█ │ █  █   │ │
│ META           620.80   +0.43%      │││ ││█│█│█ ││  │ │█││█│ │█│ ││   █│█  │████ │   ████    │
│ GOOGL          357.18   -0.48%      │││ │█│││█│█ ││││█││ █ █││█  ││       ████ █│  ││ ██  █│ │
│ AVGO           182.44   +1.12%      │││ │█     │ ███ │███ ███│█  ││     ██│ █│││  █││ █│█    │
│ TSLA           407.76   +0.30%      │││ █  █│ █││  █│█│██ █ │█││ ││ █  │█│││││█████││   │█   │
│ BRK-B          498.10   +0.08%      ││└────────── 1M ────────────┘└────────── 1M ────────────┘
│ LLY            812.35   -1.44%      ││┌─ AAPL 315.32 -0.28% ─────┐┌──────────────────────────┐
│ JPM            264.90   +0.62%      │││ █││██││ │ │ │█│██ │││││  ││                          │
│ V              341.20   +0.19%      │││ ██ █││    █│ █ █││ │ ││  ││                          │
│ XOM            118.77   -0.31%      │││ ││██  █│██ │ █││││█││││█ ││                          │
│ UNH            529.44   +2.05%      │││ █  █│  █ █│││   █ │██  │ ││                          │
│                                     │││  │ █ │█ █││  ││ │ │█│█│█ ││      enter to chart      │
│                                     │││  █   ││ █│█│██  ████││││ ││                          │
│                                     │││ │ │ │ ││█│   ██│█│█│█ ││ ││                          │
│                                     │││ █ │││ █   █│█ █│ █ │ │   ││                          │
│                                     │││  ██ │█││██████│██│  │  █ ││                          │
│                                     ││└────────── 1M ────────────┘└──────────────────────────┘
│                                     ││                                                        
│                                     ││                                                        
│                                     ││                                                        
└──────────────────────────────────────┘                                                        
 NYSE: Closed (Opens Mon 09:30)                    [1M]                  Last Refresh: 22:20:01 
 q quit  d detail  / search  f filter  s sort  c clear  tab pane  r refresh  : cmd  ? help      
```

## Detail

96x31. Left column 64 cols, fundamentals rail 32 cols.

```
┌─ AAPL · Apple Inc. · $326.77  +1.23 (+0.38%) ────────────────┐┌─ fundamentals ───────────────┐
│ ····················································  326.77 ││                              │
│ ····················································         ││ market cap           $4.71 T │
│ ····················································         ││ p/e (ttm)               32.4 │
│ ····················································         ││ p/e (fwd)               28.1 │
│ ····················································         ││ eps (ttm)               9.73 │
│ ····················································         ││ div yield              0.42% │
│ ····················································         ││ beta                    1.24 │
│ ····················································  307.87 ││                              │
│ ····················································         ││ day range                    │
│ ····················································         ││ 312.17 ─────●──────── 316.91 │
│ ····················································         ││                              │
│ ····················································         ││ 52-wk range                  │
│ ····················································         ││ 201.50 ──────────●─── 317.40 │
│ ····················································  288.98 ││                              │
└── ma50 ·── ma200 ·──────────────────────────────── 1M ───────┘│ indicators                   │
┌─ volume  48.2M · avg 52.1M ──────────────────────────────────┐│ ma50               308.44  ▲ │
│ ████████████████████████████████████████████████████     60M ││ ma200              291.02  ▲ │
│ ████████████████████████████████████████████████████         ││ rsi (14)                61.2 │
│ ████████████████████████████████████████████████████       0 ││                              │
└──────────────────────────────────────────────────────────────┘│ session                      │
┌─ rsi (14)  61.2 ─────────────────────────────────────────────┐│ open                  316.02 │
│ ────────────────────────────────────────────────────      70 ││ prev close            316.22 │
│                                                              ││ day high              316.91 │
│                                                              ││ day low               312.17 │
│ ────────────────────────────────────────────────────      30 ││                              │
└──────────────────────────────────────────────────────────────┘└──────────────────────────────┘
 NASDAQ: Closed (Opens Mon 09:30)                  [1M]                  Last Refresh: 22:20:01 
 esc back  / search  1-7 timeframe  e export  r refresh  : cmd  ? help  q quit                  
```

## Help overlay

76-col box, centered (x=10), drawn over dimmed Overview.

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

96 cols, full width.

```
┌─ settings ─ ~/.config/quotail/config.toml ───────────────────────────────────────────────────┐
│                                                                                              │
│  watchlist                                                                                   │
│  stocks              50 symbols                  enter to edit  ·  top 50 by index weight    │
│  crypto              10 symbols                  enter to edit                               │
│  indices             8 symbols                   enter to edit                               │
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
│  socket_path         ~/.local/state/quotail.sock enter to edit                               │
│                                                                                              │
│  w writes changes to config.toml                                                             │
│                                                                                              │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
 NYSE: Closed (Opens Mon 09:30)                                          Last Refresh: 22:20:01 
 j/k move  enter edit  < > change  space toggle  w write  esc back  ? help  q quit              
```

