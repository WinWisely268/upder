BINNAME := upder
PREFIX := $(HOME)/.local

.PHONY: install

install: build
	strip $(PWD)/target/release/$(BINNAME)
	install -m755 $(PWD)/target/release/$(BINNAME)  $(PREFIX)/bin/$(BINNAME)

build:
	cargo build --release

clean:
	rm -rf $(PREFIX)/bin/$(BINNAME)
