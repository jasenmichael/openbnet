# Glossary Map

## Contexts

- [wc2bne-linux](./wc2bne-linux/GLOSSARY.md): installs and runs the original game on Linux
- wc2bne-gui: a desktop window for wc2bne-linux; uses the wc2bne-linux glossary
- [openbnet-server](./openbnet-server/GLOSSARY.md): a Battle.net-compatible server
- wc2bne-wasm: the original game in a browser (planning); uses the wc2bne-linux glossary

## Relationships

- **wc2bne-linux → openbnet-server**: the game reaches an openbnet server as one of its gateways
- **wc2bne-wasm → wc2bne-linux**: shares its terms and pinned installer checksum
