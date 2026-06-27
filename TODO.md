# Lumen Roadmaps & TODOs

## Search & Memory Enhancements
- [ ] **Relative Time Filtering**: Allow phrases like "today", "yesterday", "3 hours ago" in search filters (`after`/`before`) instead of strict ISO strings.
- [ ] **Memory Importance Boosting & Decay**: 
    - Auto-increment `importance` when a memory's `access_count` hits certain milestones.
    - Implement a background task to slowly decay `importance` of old memories that haven't been accessed in weeks.
- [x] **Filtered Search**: Added type, keyword, and time filters to memories and other tools.
- [x] **Log Truncation**: Truncated verbose tool results and call args in logs.

## Core Features
- [ ] **Google Workspace**: Deepen integration with Calendar and Drive.
- [ ] **Obsidian**: Better vault indexing and bi-directional sync.
