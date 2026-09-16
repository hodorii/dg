# dg 빌드·설치
PREFIX ?= $(HOME)/.local
BINDIR  = $(PREFIX)/bin
CARGO  ?= cargo
BIN     = target/release/dg

.PHONY: all build test lint install uninstall clean

all: build

build:
	$(CARGO) build --release

test:
	$(CARGO) test

lint:
	$(CARGO) clippy -- -D warnings

install: build
	install -d $(BINDIR)
	install -m 755 $(BIN) $(BINDIR)/dg

uninstall:
	rm -f $(BINDIR)/dg

clean:
	$(CARGO) clean
