# 04: Servers tab

**What to build:** Add, remove, select and reset gateways of the chosen edition, saved together. Covers stories 44-58.

**Blocked by:** 01

**Status:** resolved

- [x] One Save sends the whole list with `gateway set`; a failed Save keeps the edits
- [x] Invalid addresses, names and offsets are explained before saving; the last gateway can't be removed
- [x] Unsaved changes are visible, can be discarded, and are not lost without asking
- [x] Reset asks first; the tab is read-only while the edition is in use, and explains itself when it is absent
- [x] Names in any language are shown and saved unchanged, never as markup
