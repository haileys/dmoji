PREFIX ?= /usr/local

.PHONY: all
all:
	cargo build --release

.PHONY: install
install:
	install -Dm 755 target/release/dmoji "$(PREFIX)/bin/dmoji"
	install -Dm 644 emoji-sequences.txt "$(PREFIX)/share/dmoji/emoji-sequences.txt"
	install -Dm 644 emoji-zwj-sequences.txt "$(PREFIX)/share/dmoji/emoji-zwj-sequences.txt"
