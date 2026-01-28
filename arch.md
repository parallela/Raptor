┌───────────────┐
│   Svelte UI   │  ← HTTP →  ┌─────────────┐
└───────────────┘             │ Rust API    │ ←→ Postgres
                              └─────┬─────┘
                                    │ HTTP + WebSocket
                                    ▼
                             ┌─────────────┐
                             │ Rust Daemon │
                             │  Container  │
                             │  Manager    │
                             └─────────────┘
