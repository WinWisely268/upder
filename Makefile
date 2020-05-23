BINNAME := upder
PREFIX := $(HOME)/.local

.PHONY: install

install: build
	install -Dm755 $(PWD)/target/release/$(BINNAME)  $(PREFIX)/bin/$(BINNAME)

build:
	cargo build --release

clean:
	rm -rf $(PREFIX)/bin/$(BINNAME)